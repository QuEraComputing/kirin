"""The `Len` dialect.

This dialect maps the `len()` call to the `Len` statement:

- The `Len` statement class.
- The lowering pass for the `len()` call.
- The concrete implementation of the `len()` call.
"""

import ast

from kirin import ir, interp, lowering
from kirin.decl import info, statement

dialect = ir.Dialect("py.len")


@statement(dialect=dialect)
class Len(ir.Statement):
    name = "len"
    traits = frozenset({ir.Pure(), ir.FromPythonCall()})
    value: ir.SSAValue = info.argument(ir.types.Any)
    result: ir.ResultValue = info.result(ir.types.Int)


@dialect.register
class Concrete(interp.MethodTable):

    @interp.impl(Len)
    def len(self, interp, frame: interp.Frame, stmt: Len):
        return (len(frame.get(stmt.value)),)


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_Call_len(
        self, state: lowering.LoweringState, node: ast.Call
    ) -> lowering.Result:
        return lowering.Result(
            state.append_stmt(Len(value=state.visit(node.args[0]).expect_one()))
        )
