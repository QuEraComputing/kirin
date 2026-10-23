"""Build native Python IR for both benchmark suites, outside their timers."""
from kirin import ir, types
from kirin.dialects import cf, func
from kirin.dialects.py.binop import Add, Sub
from kirin.dialects.py.cmp import Lt, LtE
from kirin.dialects.py.constant import Constant
from kirin.prelude import basic

from .semantic_spec import family_spec


OPERATIONS = {"add": Add, "sub": Sub}


def emit(block, stmt, type_=types.Int):
    stmt.result.type = type_
    block.stmts.append(stmt)
    return stmt.result


def method(name, names, argtypes, blocks):
    code = func.Function(
        sym_name=name,
        slots=("self", *names),
        signature=func.Signature((types.MethodType, *argtypes), types.Int),
        body=ir.Region(blocks),
    )
    return ir.Method(basic, code, arg_names=["self", *names])


def chain(block, count, path, x, y):
    values = {"x": x, "y": y}
    value, delta = values[path["start"]], values[path["delta"]]
    for _ in range(count):
        value = emit(block, OPERATIONS[path["operation"]](value, delta))
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
    elif family == "loop":
        run, skip = paths["run"], paths["skip"]
        head = ir.Block(argtypes=(types.Int, types.Int, types.Int))
        body, exit_ = ir.Block(), ir.Block()
        zero, trips, no_trips = (emit(entry, Constant(n)) for n in (0, run["trips"], skip["trips"]))
        entry.stmts.append(cf.ConditionalBranch(outer, (zero, x, trips), (zero, x, no_trips),
                                                then_successor=head, else_successor=head))
        i, acc, n = head.args
        more = emit(head, Lt(i, n), types.Bool)
        head.stmts.append(cf.ConditionalBranch(more, (), (), then_successor=body, else_successor=exit_))
        result = chain(body, steps, run, acc, y)
        one = emit(body, Constant(1))
        body.stmts.append(cf.Branch((emit(body, Add(i, one)), result, n), successor=head))
        exit_.stmts.append(func.Return(acc))
        blocks += [head, body, exit_]
    elif family in ("calls", "calls_same"):
        path = paths["all"]
        callee = ir.Block(argtypes=(types.MethodType, types.Int, types.Int))
        callee.stmts.append(func.Return(emit(callee, OPERATIONS[path["operation"]](*callee.args[1:]))))
        step = method("step", ("a", "b"), (types.Int, types.Int), [callee])
        start, delta = {"x": x, "y": y}[path["start"]], {"x": x, "y": y}[path["delta"]]
        value = start
        for _ in range(steps):
            if family == "calls":
                value = emit(entry, func.Invoke((value, delta), callee=step))
            else:
                value = emit(entry, Add(value, emit(entry, func.Invoke((start, delta), callee=step))))
        entry.stmts.append(func.Return(value))
    elif family == "recursion":
        path = paths["run"]
        rec_entry = ir.Block(argtypes=(types.MethodType, types.Int, types.Int, types.Int))
        _, n, acc, d = rec_entry.args
        base, recurse = ir.Block(), ir.Block()
        done = emit(rec_entry, LtE(n, emit(rec_entry, Constant(0))), types.Bool)
        rec_entry.stmts.append(cf.ConditionalBranch(done, (), (), then_successor=base, else_successor=recurse))
        base.stmts.append(func.Return(acc))
        rec = method("rec", ("n", "acc", "d"), (types.Int, types.Int, types.Int), [rec_entry, base, recurse])
        smaller = emit(recurse, Sub(n, emit(recurse, Constant(1))))
        following = emit(recurse, OPERATIONS[path["operation"]](acc, d))
        recurse.stmts.append(func.Return(emit(recurse, func.Invoke((smaller, following, d), callee=rec))))
        go = ir.Block(argtypes=(types.Int,))
        depth, no_depth = emit(entry, Constant(steps)), emit(entry, Constant(0))
        entry.stmts.append(cf.ConditionalBranch(outer, (depth,), (no_depth,), then_successor=go, else_successor=go))
        start, delta = {"x": x, "y": y}[path["start"]], {"x": x, "y": y}[path["delta"]]
        go.stmts.append(func.Return(emit(go, func.Invoke((go.args[0], start, delta), callee=rec))))
        blocks.append(go)
    else:
        raise ValueError(family)
    return method("benchmark", ("x", "y", "outer", "inner"), (types.Int, types.Int, types.Bool, types.Bool), blocks)
