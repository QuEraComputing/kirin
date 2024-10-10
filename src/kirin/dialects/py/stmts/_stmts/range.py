import ast

from kirin.decl import info, statement
from kirin.dialects.py import types
from kirin.dialects.py.stmts.dialect import dialect
from kirin.exceptions import DialectLoweringError
from kirin.ir import Pure, ResultValue, SSAValue, Statement
from kirin.lowering import LoweringState, Result


@statement(dialect=dialect)
class Range(Statement):
    name = "range"
    traits = frozenset({Pure()})
    start: SSAValue = info.argument(types.Int)
    stop: SSAValue = info.argument(types.Int)
    step: SSAValue = info.argument(types.Int)
    result: ResultValue = info.result(types.PyClass(range))

    @classmethod
    def from_python_call(cls, ctx: LoweringState, node: ast.Call) -> Result:
        if len(node.args) == 1:
            start = ctx.visit(ast.Constant(0)).expect_one()
            stop = ctx.visit(node.args[0]).expect_one()
            step = ctx.visit(ast.Constant(1)).expect_one()
        elif len(node.args) == 2:
            start = ctx.visit(node.args[0]).expect_one()
            stop = ctx.visit(node.args[1]).expect_one()
            step = ctx.visit(ast.Constant(1)).expect_one()
        elif len(node.args) == 3:
            start = ctx.visit(node.args[0]).expect_one()
            stop = ctx.visit(node.args[1]).expect_one()
            step = ctx.visit(node.args[2]).expect_one()
        else:
            raise DialectLoweringError("range() takes 1-3 arguments")

        return Result(ctx.append_stmt(cls(start, stop, step)))
