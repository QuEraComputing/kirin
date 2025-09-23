"""The slice dialect for Python.

This dialect provides a `Slice` statement that represents a slice object in Python:

- The `Slice` statement class.
- The lowering pass for the `slice` call.
- The concrete implementation of the `slice` call.
- The type inference implementation of the `slice` call.
"""

import ast
from dataclasses import dataclass

from kirin import ir, types, interp, lowering
from kirin.decl import info, statement
from kirin.print.printer import Printer
from kirin.dialects.py.constant import Constant

dialect = ir.Dialect("py.slice")


@dataclass(frozen=True)
class SliceLowering(lowering.FromPythonCall["Slice"]):

    def lower(
        self, stmt: type["Slice"], state: lowering.State, node: ast.Call
    ) -> lowering.Result:
        return _lower_slice(state, node)


T = types.TypeVar("T")


@statement(dialect=dialect, init=False)
class Slice(ir.Statement):
    name = "slice"
    traits = frozenset({ir.Pure(), SliceLowering()})
    start: ir.SSAValue = info.argument(T | types.NoneType)
    stop: ir.SSAValue = info.argument(T | types.NoneType)
    step: ir.SSAValue = info.argument(T | types.NoneType)
    result: ir.ResultValue = info.result(types.Slice[T])

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
                result_type = types.Slice[stop.type]
        else:
            result_type = types.Slice[start.type]

        super().__init__(
            args=(start, stop, step),
            result_types=[result_type],
            args_slice={"start": 0, "stop": 1, "step": 2},
        )


@dataclass
class SliceAttribute(ir.Data[slice]):

    start: int | None
    stop: int | None
    step: int | None

    def __post_init__(self) -> None:
        if self.start is None and self.step is None:
            self.type = types.Slice[types.Literal(self.stop)]
        else:
            self.type = types.Slice3[
                types.Literal(self.start),
                types.Literal(self.stop),
                types.Literal(self.step),
            ]

    def unwrap(self):
        return slice(self.start, self.stop, self.step)

    def __hash__(self):
        return hash((type(self), self.start, self.stop, self.step))

    def print_impl(self, printer: Printer) -> None:
        return printer.plain_print(f"slice({self.start}, {self.stop}, {self.step})")


@dialect.register
class Concrete(interp.MethodTable):

    @interp.impl(Slice)
    def _slice(self, interp, frame: interp.Frame, stmt: Slice):
        start, stop, step = frame.get_values(stmt.args)
        if start is None and step is None:
            return (SliceAttribute(None, stop, None),)
        elif step is None:
            return (SliceAttribute(start, stop, None),)
        else:
            return (SliceAttribute(start, stop, step),)


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_Slice(self, state: lowering.State, node: ast.Slice) -> lowering.Result:
        def value_or_none(expr: ast.expr | None) -> ir.SSAValue:
            if expr is not None:
                return state.lower(expr).expect_one()
            else:
                return state.current_frame.push(Constant(None)).result

        lower = value_or_none(node.lower)
        upper = value_or_none(node.upper)
        step = value_or_none(node.step)

        return state.current_frame.push(Slice(start=lower, stop=upper, step=step))

    @lowering.akin(slice)
    def lower_Call_slice(
        self, state: lowering.State, node: ast.Call
    ) -> lowering.Result:
        return _lower_slice(state, node)


def _lower_slice(state: lowering.State, node: ast.Call) -> lowering.Result:
    if len(node.args) == 1:
        start = state.lower(ast.Constant(None)).expect_one()
        stop = state.lower(node.args[0]).expect_one()
        step = state.lower(ast.Constant(None)).expect_one()
    elif len(node.args) == 2:
        start = state.lower(node.args[0]).expect_one()
        stop = state.lower(node.args[1]).expect_one()
        step = state.lower(ast.Constant(None)).expect_one()
    elif len(node.args) == 3:
        start = state.lower(node.args[0]).expect_one()
        stop = state.lower(node.args[1]).expect_one()
        step = state.lower(node.args[2]).expect_one()
    else:
        raise lowering.BuildError("slice() takes 1-3 arguments")

    return state.current_frame.push(Slice(start, stop, step))
