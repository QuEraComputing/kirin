from kirin import ir, types
from kirin.dialects.py import constant
from kirin.rewrite.abc import RewriteRule
from kirin.rewrite.result import RewriteResult
from kirin.dialects.ilist.stmts import IListType
from kirin.dialects.ilist.runtime import IList


class List2IList(RewriteRule):

    def rewrite_Block(self, node: ir.Block) -> RewriteResult:
        has_done_something = False
        for arg in node.args:
            has_done_something = self._rewrite_SSAValue_type(arg)
        return RewriteResult(has_done_something=has_done_something)

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        has_done_something = False
        if isinstance(node, constant.Constant) and isinstance(node.value, list):
            eltype = self._eltype(node.result.type)
            node.replace_by(
                constant.Constant(value=IList(data=node.value, elem=eltype))
            )

        for result in node.results:
            has_done_something = self._rewrite_SSAValue_type(result)

        return RewriteResult(has_done_something=has_done_something)

    def _rewrite_SSAValue_type(self, value: ir.SSAValue):
        # NOTE: cannot use issubseteq here because type can be Bottom
        if isinstance(value.type, types.Generic) and issubclass(
            value.type.body.typ, list
        ):
            value.type = IListType[value.type.vars[0], types.Any]
            return True

        elif isinstance(value.type, types.PyClass) and issubclass(value.type.typ, list):
            value.type = IListType[types.Any, types.Any]
            return True
        return False

    def _eltype(self, type: types.TypeAttribute):
        if isinstance(type, types.Generic) and issubclass(type.body.typ, (list, IList)):
            return type.vars[0]
        return types.Any
