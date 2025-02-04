"""The range dialect for Python.

This dialect models the builtin `range()` function in Python.

The dialect includes:
- The `Range` statement class.
- The lowering pass for the `range()` function.

This dialect does not include a concrete implementation or type inference
for the `range()` function. One needs to use other dialect for the concrete
implementation and type inference, e.g., `ilist` dialect.
"""

import ast
from dataclasses import dataclass

from kirin import ir, types, lowering, exceptions
from kirin.decl import info, statement

dialect = ir.Dialect("py.range")


@dataclass(frozen=True)
class RangeLowering(ir.FromPythonCall["Range"]):

    def lower(
        self, stmt: type["Range"], state: lowering.LoweringState, node: ast.Call
    ) -> lowering.Result:
        return _lower_range(state, node)


@statement(dialect=dialect)
class Range(ir.Statement):
    name = "range"
    traits = frozenset({ir.Pure(), RangeLowering()})
    start: ir.SSAValue = info.argument(types.Int)
    stop: ir.SSAValue = info.argument(types.Int)
    step: ir.SSAValue = info.argument(types.Int)
    result: ir.ResultValue = info.result(types.PyClass(range))


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_Call_range(
        self, state: lowering.LoweringState, node: ast.Call
    ) -> lowering.Result:
        return _lower_range(state, node)


def _lower_range(state: lowering.LoweringState, node: ast.Call) -> lowering.Result:
    if len(node.args) == 1:
        start = state.visit(ast.Constant(0)).expect_one()
        stop = state.visit(node.args[0]).expect_one()
        step = state.visit(ast.Constant(1)).expect_one()
    elif len(node.args) == 2:
        start = state.visit(node.args[0]).expect_one()
        stop = state.visit(node.args[1]).expect_one()
        step = state.visit(ast.Constant(1)).expect_one()
    elif len(node.args) == 3:
        start = state.visit(node.args[0]).expect_one()
        stop = state.visit(node.args[1]).expect_one()
        step = state.visit(node.args[2]).expect_one()
    else:
        raise exceptions.DialectLoweringError("range() takes 1-3 arguments")

    return lowering.Result(state.append_stmt(Range(start, stop, step)))
