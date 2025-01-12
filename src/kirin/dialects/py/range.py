import ast

from kirin import ir, types, interp, lowering, exceptions
from kirin.decl import info, statement

dialect = ir.Dialect("py.range")


@statement(dialect=dialect)
class Range(ir.Statement):
    name = "range"
    traits = frozenset({ir.Pure()})
    start: ir.SSAValue = info.argument(types.Int)
    stop: ir.SSAValue = info.argument(types.Int)
    step: ir.SSAValue = info.argument(types.Int)
    result: ir.ResultValue = info.result(types.PyClass(range))

    @classmethod
    def from_python_call(
        cls, state: lowering.LoweringState, node: ast.Call
    ) -> lowering.Result:
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

        return lowering.Result(state.append_stmt(cls(start, stop, step)))


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_Call_range(
        self, state: lowering.LoweringState, node: ast.Call
    ) -> lowering.Result:
        return Range.from_python_call(state, node)


@dialect.register
class Concrete(interp.MethodTable):

    @interp.impl(Range)
    def _range(self, interp, frame: interp.Frame, stmt: Range):
        return (range(*frame.get_values(stmt.args)),)
