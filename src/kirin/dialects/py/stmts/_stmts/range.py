import ast

from kirin.ir import Pure, SSAValue, Statement, ResultValue, types
from kirin.decl import info, statement
from kirin.lowering import Result, LoweringState
from kirin.exceptions import DialectLoweringError
from kirin.dialects.py.stmts.dialect import dialect


@statement(dialect=dialect)
class Range(Statement):
    name = "range"
    traits = frozenset({Pure()})
    start: SSAValue = info.argument(types.Int)
    stop: SSAValue = info.argument(types.Int)
    step: SSAValue = info.argument(types.Int)
    result: ResultValue = info.result(types.PyClass(range))

    @classmethod
    def from_python_call(cls, state: LoweringState, node: ast.Call) -> Result:
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
            raise DialectLoweringError("range() takes 1-3 arguments")

        return Result(state.append_stmt(cls(start, stop, step)))
