import ast
from dataclasses import dataclass

from kirin import ir, types, interp, lowering, exceptions
from kirin.decl import info, statement
from kirin.analysis.typeinfer import TypeInference
from kirin.dialects.py.constant import Constant

dialect = ir.Dialect("py.slice")


@dataclass(frozen=True)
class SliceLowering(ir.FromPythonCall["Slice"]):

    def lower(
        self, stmt: type["Slice"], state: lowering.LoweringState, node: ast.Call
    ) -> lowering.Result:
        return _lower_slice(state, node)


@statement(dialect=dialect, init=False)
class Slice(ir.Statement):
    name = "slice"
    traits = frozenset({ir.Pure(), SliceLowering()})
    start: ir.SSAValue = info.argument(types.Any)
    stop: ir.SSAValue = info.argument(types.Any)
    step: ir.SSAValue = info.argument(types.Any)
    result: ir.ResultValue = info.result(types.Slice)

    def __init__(
        self, start: ir.SSAValue, stop: ir.SSAValue, step: ir.SSAValue
    ) -> None:
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


@dialect.register
class Concrete(interp.MethodTable):

    @interp.impl(Slice)
    def _slice(self, interp, frame: interp.Frame, stmt: Slice):
        start, stop, step = frame.get_values(stmt.args)
        if start is None and step is None:
            return (slice(stop),)
        elif step is None:
            return (slice(start, stop),)
        else:
            return (slice(start, stop, step),)


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_Slice(
        self, state: lowering.LoweringState, node: ast.Slice
    ) -> lowering.Result:
        def value_or_none(expr: ast.expr | None) -> ir.SSAValue:
            if expr is not None:
                return state.visit(expr).expect_one()
            else:
                return state.append_stmt(Constant(None)).result

        lower = value_or_none(node.lower)
        upper = value_or_none(node.upper)
        step = value_or_none(node.step)
        return lowering.Result(
            state.append_stmt(Slice(start=lower, stop=upper, step=step))
        )

    def lower_Call_slice(
        self, state: lowering.LoweringState, node: ast.Call
    ) -> lowering.Result:
        return _lower_slice(state, node)


@dialect.register(key="typeinfer")
class TypeInfer(interp.MethodTable):

    @interp.impl(Slice)
    def slice(
        self,
        interp: TypeInference,
        frame: interp.Frame[types.TypeAttribute],
        stmt: Slice,
    ):
        start, stop, step = frame.get_values(stmt.args)
        if interp.is_const(start) and interp.is_const(stop) and interp.is_const(step):
            return (
                types.Hinted(
                    stmt.result.type,
                    slice(start.data.data, stop.data.data, step.data.data),
                ),
            )

        return (stmt.result.type,)


def _lower_slice(state: lowering.LoweringState, node: ast.Call) -> lowering.Result:
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
        raise exceptions.DialectLoweringError("slice() takes 1-3 arguments")

    return lowering.Result(state.append_stmt(Slice(start, stop, step)))
