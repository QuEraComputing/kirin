"""The `Len` dialect.

This dialect maps the `len()` call to the `Len` statement:

- The `Len` statement class.
- The lowering pass for the `len()` call.
- The concrete implementation of the `len()` call.
"""

import ast

from kirin import ir, types, interp, lowering
from kirin.decl import info, statement
from kirin.analysis import const

dialect = ir.Dialect("py.len")


@statement(dialect=dialect)
class Len(ir.Statement):
    name = "len"
    traits = frozenset({ir.Pure(), ir.FromPythonCall()})
    value: ir.SSAValue = info.argument(types.Any)
    result: ir.ResultValue = info.result(types.Int)


@dialect.register
class Concrete(interp.MethodTable):

    @interp.impl(Len)
    def len(self, interp, frame: interp.Frame, stmt: Len):
        return (len(frame.get(stmt.value)),)


@dialect.register(key="constprop")
class ConstProp(interp.MethodTable):

    @interp.impl(Len)
    def len(self, interp, frame: interp.Frame, stmt: Len):
        value = frame.get(stmt.value)
        if isinstance(value, const.Value):
            return (const.Value(len(value.data)),)
        elif isinstance(value, const.PartialTuple):
            return (const.Value(len(value.data)),)
        else:
            return (const.Result.top(),)


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_Call_len(
        self, state: lowering.LoweringState, node: ast.Call
    ) -> lowering.Result:
        return lowering.Result(
            state.append_stmt(Len(value=state.visit(node.args[0]).expect_one()))
        )
