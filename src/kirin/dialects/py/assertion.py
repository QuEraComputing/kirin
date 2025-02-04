"""Assertion dialect for Python.

This module contains the dialect for the Python `assert` statement, including:

- The `Assert` statement class.
- The lowering pass for the `assert` statement.
- The concrete implementation of the `assert` statement.
- The type inference implementation of the `assert` statement.
- The Julia emitter for the `assert` statement.

This dialect maps `ast.Assert` nodes to the `Assert` statement.
"""

import ast

from kirin import ir, types, interp, lowering
from kirin.decl import info, statement
from kirin.emit import EmitStrFrame, julia
from kirin.print import Printer

dialect = ir.Dialect("py.assert")


@statement(dialect=dialect)
class Assert(ir.Statement):
    condition: ir.SSAValue
    message: ir.SSAValue = info.argument(types.String)

    def print_impl(self, printer: Printer) -> None:
        with printer.rich(style="keyword"):
            printer.print_name(self)

        printer.plain_print(" ")
        printer.print(self.condition)

        if self.message:
            printer.plain_print(", ")
            printer.print(self.message)


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_Assert(
        self, state: lowering.LoweringState, node: ast.Assert
    ) -> lowering.Result:
        from kirin.dialects.py.constant import Constant

        cond = state.visit(node.test).expect_one()
        if node.msg:
            message = state.visit(node.msg).expect_one()
            state.append_stmt(Assert(condition=cond, message=message))
        else:
            message_stmt = state.append_stmt(Constant(""))
            state.append_stmt(Assert(condition=cond, message=message_stmt.result))
        return lowering.Result()


@dialect.register
class Concrete(interp.MethodTable):

    @interp.impl(Assert)
    def assert_stmt(
        self, interp_: interp.Interpreter, frame: interp.Frame, stmt: Assert
    ):
        if frame.get(stmt.condition) is True:
            return ()

        if stmt.message:
            raise interp.WrapException(AssertionError(frame.get(stmt.message)))
        else:
            raise interp.WrapException(AssertionError("Assertion failed"))


@dialect.register(key="typeinfer")
class TypeInfer(interp.MethodTable):

    @interp.impl(Assert)
    def assert_stmt(self, interp, frame, stmt: Assert):
        return (types.Bottom,)


@dialect.register(key="emit.julia")
class EmitJulia(interp.MethodTable):

    @interp.impl(Assert)
    def emit_assert(self, interp: julia.EmitJulia, frame: EmitStrFrame, stmt: Assert):
        interp.writeln(
            frame, f"@assert {frame.get(stmt.condition)} {frame.get(stmt.message)}"
        )
        return ()
