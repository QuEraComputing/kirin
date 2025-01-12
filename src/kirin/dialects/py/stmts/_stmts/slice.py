import ast

from kirin.ir import Pure, SSAValue, Statement, ResultValue, types
from kirin.decl import info, statement
from kirin.lowering import Result, LoweringState
from kirin.exceptions import DialectLoweringError
from kirin.dialects.py.stmts.dialect import dialect


@statement(dialect=dialect, init=False)
class Slice(Statement):
    name = "slice"
    traits = frozenset({Pure()})
    start: SSAValue = info.argument(types.Any)
    stop: SSAValue = info.argument(types.Any)
    step: SSAValue = info.argument(types.Any)
    result: ResultValue = info.result(types.Slice)

    def __init__(self, start: SSAValue, stop: SSAValue, step: SSAValue) -> None:
        if not (
            isinstance(stop.type, types.TypeAttribute)
            and isinstance(start.type, types.TypeAttribute)
        ):
            result_type = types.Bottom
        elif start.type.is_subseteq(types.NoneType):
            if stop.type.is_subseteq(types.NoneType):
                result_type = types.Bottom
            else:
                result_type = types.Slice[types.unwrap_hinted(stop.type)]
        else:
            result_type = types.Slice[types.unwrap_hinted(start.type)]

        super().__init__(
            args=(start, stop, step),
            result_types=[result_type],
            args_slice={"start": 0, "stop": 1, "step": 2},
        )

    @classmethod
    def from_python_call(cls, state: LoweringState, node: ast.Call) -> Result:
        if len(node.args) == 1:
            start = state.visit(ast.Constant(None)).expect_one()
            stop = state.visit(node.args[0]).expect_one()
            step = state.visit(ast.Constant(None)).expect_one()
        elif len(node.args) == 2:
            start = state.visit(node.args[0]).expect_one()
            stop = state.visit(node.args[1]).expect_one()
            step = state.visit(ast.Constant(None)).expect_one()
        elif len(node.args) == 3:
            start = state.visit(node.args[0]).expect_one()
            stop = state.visit(node.args[1]).expect_one()
            step = state.visit(node.args[2]).expect_one()
        else:
            raise DialectLoweringError("slice() takes 1-3 arguments")

        return Result(state.append_stmt(cls(start, stop, step)))
