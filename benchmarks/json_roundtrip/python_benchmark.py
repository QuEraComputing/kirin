"""Python half of the JSON-only benchmark. Run using the pinned baseline's Python."""
import argparse
import collections
import gc
import json
import time

from kirin import ir, types
from kirin.dialects import cf, func, scf
from kirin.dialects.py.binop import Add
from kirin.prelude import basic


def chain(block, count, value, other):
    for _ in range(count):
        stmt = Add(value, other)
        stmt.result.type = types.Int
        block.stmts.append(stmt)
        value = stmt.result
    return value


def workload(family, size):
    # The first block argument is Python Method's unused implicit self. Rust
    # carries the same unused argument so argument counts/shapes match.
    entry = ir.Block(argtypes=(types.Int, types.Int, types.Int))
    _, x, cond = entry.args
    blocks = [entry]
    if family == "chain":
        entry.stmts.append(func.Return(chain(entry, size, x, x)))
    elif family == "diamond":
        left = ir.Block(argtypes=(types.Int,))
        right = ir.Block(argtypes=(types.Int,))
        merge = ir.Block(argtypes=(types.Int,))
        entry.stmts.append(cf.ConditionalBranch(cond, (x,), (x,), then_successor=left, else_successor=right))
        l = chain(left, size // 2, left.args[0], left.args[0])
        left.stmts.append(cf.Branch((l,), successor=merge))
        r = chain(right, size // 2, right.args[0], right.args[0])
        right.stmts.append(cf.Branch((r,), successor=merge))
        merge.stmts.append(func.Return(merge.args[0]))
        blocks += [left, right, merge]
    elif family == "nested_if":
        def leaf():
            block = ir.Block()
            block.stmts.append(scf.Yield(chain(block, size // 4, x, x)))
            return block

        def inner():
            block = ir.Block()
            stmt = scf.IfElse(cond, leaf(), leaf())
            block.stmts.append(stmt)
            block.stmts.append(scf.Yield(stmt.results[0]))
            return block

        stmt = scf.IfElse(cond, inner(), inner())
        entry.stmts.append(stmt)
        entry.stmts.append(func.Return(stmt.results[0]))
    else:
        raise ValueError(family)
    code = func.Function(
        sym_name="benchmark",
        slots=("self", "x", "cond"),
        signature=func.Signature((types.Int, types.Int, types.Int), types.Int),
        body=ir.Region(blocks),
    )
    return ir.Method(basic, code, arg_names=["self", "x", "cond"])


def describe(code):
    counts = collections.Counter()
    blocks = 0

    def walk(stmt):
        nonlocal blocks
        if stmt is not code:
            names = {Add: "add", cf.Branch: "br", cf.ConditionalBranch: "cond_br",
                     scf.IfElse: "if", scf.Yield: "yield", func.Return: "ret"}
            counts[names[type(stmt)]] += 1
        for region in stmt.regions:
            blocks += len(region.blocks)
            for block in region.blocks:
                for child in block.stmts:
                    walk(child)
    walk(code)
    return dict(sorted(counts.items())), blocks


def shape(code):
    """Normalize actual SSA IDs, types, CFG targets and nested bodies."""
    blocks = []
    def visit(block):
        blocks.append(block)
        for stmt in block.stmts:
            for region in stmt.regions:
                for child in region.blocks:
                    visit(child)
    for block in code.body.blocks:
        visit(block)
    block_ids = {block: i for i, block in enumerate(blocks)}
    values = {}
    for block in blocks:
        for value in block.args:
            values[value] = len(values)
        for stmt in block.stmts:
            for value in stmt.results:
                values[value] = len(values)
    def typed(value):
        assert value.type == types.Int, value.type
        return {"id": values[value], "type": "int"}
    names = {Add: "add", cf.Branch: "br", cf.ConditionalBranch: "cond_br",
             scf.IfElse: "if", scf.Yield: "yield", func.Return: "ret"}
    return [{"arguments": [typed(v) for v in block.args],
             "ops": [{"kind": names[type(stmt)], "inputs": [values[v] for v in stmt.args],
                      "outputs": [typed(v) for v in stmt.results],
                      "targets": [block_ids[b] for b in stmt.successors],
                      "bodies": [block_ids[b] for r in stmt.regions for b in r.blocks]}
                     for stmt in block.stmts]}
            for block in blocks]

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("family", choices=("chain", "diamond", "nested_if"))
    parser.add_argument("adds", type=int)
    parser.add_argument("repeats", type=int, nargs="?", default=30)
    parser.add_argument("warmups", type=int, nargs="?", default=5)
    args = parser.parse_args()
    assert args.adds > 0 and args.adds % 4 == 0 and args.repeats > 0
    program = workload(args.family, args.adds)
    # These APIs include BOTH semantic IR encoding/decoding and actual JSON.
    encoded = basic.encode_json(program)
    restored = basic.decode_json(encoded)
    assert restored.code.is_structurally_equal(program.code)
    operations, blocks = describe(program.code)
    assert operations["add"] == args.adds
    assert describe(restored.code) == (operations, blocks)

    normalized = shape(program.code)
    assert normalized == shape(restored.code)

    def roundtrip():
        payload = basic.encode_json(program)
        return payload, basic.decode_json(payload)

    for _ in range(args.warmups):
        output = roundtrip()
        del output
        gc.collect()
    samples = []
    for _ in range(args.repeats):
        # Python IR owns cyclic links. Collect between samples rather than
        # measuring unrelated reclamation of previous decoded IRs.
        gc.collect()
        enabled = gc.isenabled()
        gc.disable()
        try:
            start = time.perf_counter()
            output = roundtrip()
            elapsed = time.perf_counter() - start
        finally:
            if enabled:
                gc.enable()
        samples.append(elapsed)
        del output  # output lifetime/reclamation is outside the timer
    print(json.dumps({"implementation": "python", "family": args.family,
                      "adds": args.adds, "operations": operations, "blocks": blocks,
                      "shape": normalized, "json_bytes": len(encoded.encode()), "warmups": args.warmups,
                      "samples_seconds": samples}))


if __name__ == "__main__":
    main()
