"""The `Len` dialect.

This dialect maps the `len()` call to the `Len` statement:

- The `Len` statement class.
- The lowering pass for the `len()` call.
- The concrete implementation of the `len()` call.
"""

import ast
from functools import cached_property

from kirin import ir, types, interp, lowering
from kirin.decl import info, statement
from kirin.analysis import const
from kirin.rewrite.abc import RewriteRule, RewriteResult

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


class InferLen(RewriteRule):

    @cached_property
    def Constant(self):
        # Avoid circular import caching the result
        from kirin.dialects.py.constant import Constant

        return Constant

    @cached_property
    def IListType(self):
        # Avoid circular import caching the result
        from kirin.dialects.ilist import IListType

        return IListType

    def _get_collection_len(self, collection: ir.SSAValue):
        coll_type = collection.type

        if not isinstance(coll_type, types.Generic):
            return None

        if (
            coll_type.is_subseteq(self.IListType)
            and isinstance(coll_type.vars[1], types.Literal)
            and isinstance(coll_type.vars[1].data, int)
        ):
            return coll_type.vars[1].data
        else:
            return None

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, Len):
            return RewriteResult()

        if (coll_len := self._get_collection_len(node.value)) is None:
            return RewriteResult()

        node.replace_by(self.Constant(coll_len))
        return RewriteResult(has_done_something=True)
