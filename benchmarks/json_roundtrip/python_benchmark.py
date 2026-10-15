"""Python half of the JSON-only benchmark. Run using the pinned baseline's Python."""
import argparse
import collections
import gc
import json
from pathlib import Path
import time

from rich.console import Console

import kirin
from kirin import ir, types
from kirin.dialects import cf, func
from kirin.dialects.py.binop import Add, Sub
from kirin.prelude import basic

from validation import validate_shape
from semantic_spec import family_spec, validate_runtime


def chain(block, count, path, x, y):
    values = {"x": x, "y": y}
    value, delta = values[path["start"]], values[path["delta"]]
    operation = {"add": Add, "sub": Sub}[path["operation"]]
    for _ in range(count):
        stmt = operation(value, delta)
        stmt.result.type = types.Int
        block.stmts.append(stmt)
        value = stmt.result
    return value


def workload(family, size):
    specification = family_spec(family, size)
    paths = {path["name"]: path for path in specification["paths"]}
    steps = size // specification["steps_divisor"]
    # Python's Method wrapper needs self; Rust does not need this extra argument.
    entry = ir.Block(argtypes=(types.MethodType, types.Int, types.Int, types.Bool, types.Bool))
    _, x, y, outer, inner = entry.args
    blocks = [entry]
    if family == "chain":
        entry.stmts.append(func.Return(chain(entry, steps, paths["all"], x, y)))
    elif family == "diamond":
        left, right = [ir.Block(argtypes=(types.Int, types.Int)) for _ in range(2)]
        merge = ir.Block(argtypes=(types.Int,))
        entry.stmts.append(cf.ConditionalBranch(outer, (x, y), (x, y), then_successor=left, else_successor=right))
        for block, name in [(left, "left"), (right, "right")]:
            result = chain(block, steps, paths[name], *block.args)
            block.stmts.append(cf.Branch((result,), successor=merge))
        merge.stmts.append(func.Return(merge.args[0]))
        blocks += [left, right, merge]
    elif family == "nested_cfg":
        left, right = [ir.Block(argtypes=(types.Int, types.Int)) for _ in range(2)]
        a, b, c, d = [ir.Block(argtypes=(types.Int, types.Int)) for _ in range(4)]
        left_merge, right_merge, merge = [ir.Block(argtypes=(types.Int,)) for _ in range(3)]
        entry.stmts.append(cf.ConditionalBranch(outer, (x, y), (x, y), then_successor=left, else_successor=right))
        left.stmts.append(cf.ConditionalBranch(inner, tuple(left.args), tuple(left.args), then_successor=a, else_successor=b))
        right.stmts.append(cf.ConditionalBranch(inner, tuple(right.args), tuple(right.args), then_successor=c, else_successor=d))
        for leaf, name, target in [(a, "a", left_merge), (b, "b", left_merge), (c, "c", right_merge), (d, "d", right_merge)]:
            result = chain(leaf, steps, paths[name], *leaf.args)
            leaf.stmts.append(cf.Branch((result,), successor=target))
        left_merge.stmts.append(cf.Branch((left_merge.args[0],), successor=merge))
        right_merge.stmts.append(cf.Branch((right_merge.args[0],), successor=merge))
        merge.stmts.append(func.Return(merge.args[0]))
        blocks += [left, right, a, b, c, d, left_merge, right_merge, merge]
    else:
        raise ValueError(family)
    code = func.Function(
        sym_name="benchmark",
        slots=("self", "x", "y", "outer", "inner"),
        signature=func.Signature((types.MethodType, types.Int, types.Int, types.Bool, types.Bool), types.Int),
        body=ir.Region(blocks),
    )
    return ir.Method(basic, code, arg_names=["self", "x", "y", "outer", "inner"])


def describe(code):
    counts = collections.Counter()
    blocks = 0

    def walk(stmt):
        nonlocal blocks
        if stmt is not code:
            names = {Add: "add", Sub: "sub", cf.Branch: "br", cf.ConditionalBranch: "cond_br",
                     func.Return: "ret"}
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
        assert value.type in (types.Int, types.Bool, types.MethodType), value.type
        return {"id": values[value], "type": "method" if value.type == types.MethodType else ("bool" if value.type == types.Bool else "int")}
    names = {Add: "add", Sub: "sub", cf.Branch: "br", cf.ConditionalBranch: "cond_br",
             func.Return: "ret"}
    return [{"arguments": [typed(v) for v in block.args],
             "ops": [{"kind": names[type(stmt)], "inputs": [values[v] for v in stmt.args],
                      "outputs": [typed(v) for v in stmt.results],
                      "targets": [block_ids[b] for b in stmt.successors],
                      "bodies": [block_ids[b] for r in stmt.regions for b in r.blocks]}
                     for stmt in block.stmts]}
            for block in blocks]

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("family", choices=("chain", "diamond", "nested_cfg"))
    parser.add_argument("size", type=int)
    parser.add_argument("repeats", type=int, nargs="?", default=30)
    parser.add_argument("warmups", type=int, nargs="?", default=5)
    parser.add_argument("export", type=Path, nargs="?")
    args = parser.parse_args()
    assert args.size > 0 and args.size % 4 == 0 and args.repeats > 0
    program = workload(args.family, args.size)
    # These APIs include BOTH semantic IR encoding/decoding and actual JSON.
    program.code.verify()
    program.code.verify_type()
    encoded = basic.encode_json(program)
    restored = basic.decode_json(encoded)
    restored.code.verify()
    restored.code.verify_type()
    assert restored.code.is_structurally_equal(program.code)
    operations, blocks = describe(program.code)
    assert operations.get("add", 0) + operations.get("sub", 0) == args.size
    assert describe(restored.code) == (operations, blocks)

    normalized = shape(program.code)
    validate_shape(normalized)
    validate_shape(shape(restored.code))
    assert normalized == shape(restored.code)

    semantic_cases = validate_runtime(program, restored, args.family, args.size)

    if args.export:
        args.export.mkdir(parents=True, exist_ok=True)
        (args.export / f"{args.family}-{args.size}.python.json").write_text(encoded)
        printed = program.code.print_str(
            console=Console(color_system=None, force_terminal=False, width=160),
            show_indent_mark=False,
        )
        (args.export / f"{args.family}-{args.size}.python.kirin").write_text(printed)

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
                      "kirin_source": str(Path(kirin.__file__).resolve()),
                      "size": args.size, "semantic_cases": semantic_cases, "operations": operations, "blocks": blocks,
                      "shape": normalized, "json_bytes": len(encoded.encode()), "warmups": args.warmups,
                      "samples_seconds": samples}))


if __name__ == "__main__":
    main()
