"""Constant statement for Python dialect.

This module contains the dialect for the Python `constant` statement, including:

- The `Constant` statement class.
- The lowering pass for the `constant` statement.
- The concrete implementation of the `constant` statement.
- The Julia emitter for the `constant` statement.

This dialect maps `ast.Constant` nodes to the `Constant` statement.
"""

from __future__ import annotations

import ast
from typing import Generic, TypeVar

from kirin import ir, emit, types, interp, lowering
from kirin.decl import info, statement
from kirin.print import Printer

dialect = ir.Dialect("py.constant")

T = TypeVar("T", covariant=True)


@statement(dialect=dialect)
class Constant(ir.Statement, Generic[T]):
    name = "constant"
    traits = frozenset({ir.Pure(), ir.ConstantLike(), lowering.FromPythonCall()})
    value: ir.Data[T] = info.attribute()
    result: ir.ResultValue = info.result()

    # NOTE: we allow py.Constant take data.PyAttr too
    def __init__(self, value: T | ir.Data[T]) -> None:
        if not isinstance(value, ir.Data):
            value = ir.PyAttr(value)
        super().__init__(
            attributes={"value": value},
            result_types=(value.type,),
        )

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self)
        printer.plain_print(" ")
        if isinstance(self.value, ir.PyAttr):
            printer.plain_print(repr(self.value.data))
        else:  # other attributes
            printer.plain_print(repr(self.value))
        with printer.rich(style="comment"):
            printer.plain_print(" : ")
            printer.print(self.result.type)

    def check_type(self) -> None:
        if not isinstance(self.result.type, types.TypeAttribute):
            raise TypeError(
                f"Expected result type to be PyType, got {self.result.type}"
            )


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_Constant(
        self, state: lowering.State, node: ast.Constant
    ) -> lowering.Result:
        return state.current_frame.push(
            Constant(node.value),
        )


@dialect.register
class Concrete(interp.MethodTable):

    @interp.impl(Constant)
    def constant(self, interp, frame: interp.Frame, stmt: Constant):
        return (stmt.value.unwrap(),)


@dialect.register(key="emit.julia")
class Julia(interp.MethodTable):

    @interp.impl(Constant)
    def constant(self, emit: emit.Julia, frame: emit.JuliaFrame, stmt: Constant):
        return (emit.get_attribute(frame, stmt.value),)
