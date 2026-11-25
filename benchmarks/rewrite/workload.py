"""Python IR for the rewrite shapes, mirroring `rust/workload.rs`."""
from kirin import ir, types
from kirin.dialects import func
from kirin.dialects.py.binop import Add, Sub
from kirin.dialects.py.constant import Constant
from kirin.dialects.py.unary import USub

from common import python_workload as shared


def straight_line(size, line, returns_x):
    """`@benchmark` holding `size` statements `line(previous, x, y)`, returning x or the last result."""
    entry = ir.Block(argtypes=(types.MethodType, types.Int, types.Int, types.Bool, types.Bool))
    _, x, y, _, _ = entry.args
    previous = x
    for _ in range(size):
        previous = shared.emit(entry, line(previous, x, y))
    entry.stmts.append(func.Return(x if returns_x else previous))
    return shared.method("benchmark", ("x", "y", "outer", "inner"),
                         (types.Int, types.Int, types.Bool, types.Bool), [entry])


def workload(shape, size):
    if shape in ("chain", "diamond", "nested_cfg"):
        return shared.workload(shape, size)
    if shape == "neg_chain":
        return straight_line(size, lambda previous, x, y: USub(previous), returns_x=False)
    if shape == "dead_independent":
        return straight_line(size, lambda previous, x, y: Add(x, y), returns_x=True)
    if shape == "dead_chain":
        return straight_line(size, lambda previous, x, y: Add(previous, y), returns_x=True)
    if shape.startswith("folds_"):
        folds = int(shape.split("_")[1])
        entry = ir.Block(argtypes=(types.MethodType, types.Int, types.Int, types.Bool, types.Bool))
        _, previous, y, _, _ = entry.args
        for _ in range(size):
            previous = shared.emit(entry, Add(previous, y))
        for _ in range(2 * folds):
            previous = shared.emit(entry, USub(previous))
        entry.stmts.append(func.Return(previous))
        return shared.method("benchmark", ("x", "y", "outer", "inner"),
                             (types.Int, types.Int, types.Bool, types.Bool), [entry])
    if shape.startswith("unroll_"):
        k = int(shape[len("unroll_"):])
        entry = ir.Block(argtypes=(types.MethodType, types.Int, types.Int, types.Bool, types.Bool))
        previous = entry.args[1]
        for _ in range(size // 2):
            previous = shared.emit(entry, Sub(previous, shared.emit(entry, Constant(k))))
        entry.stmts.append(func.Return(previous))
        return shared.method("benchmark", ("x", "y", "outer", "inner"),
                             (types.Int, types.Int, types.Bool, types.Bool), [entry])
    raise ValueError(shape)
