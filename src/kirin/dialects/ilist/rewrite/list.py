from kirin import ir
from kirin.rewrite.abc import RewriteRule
from kirin.rewrite.result import RewriteResult
from kirin.dialects.ilist.stmts import IListType


class List2IList(RewriteRule):

    def rewrite_Block(self, node: ir.Block) -> RewriteResult:
        has_done_something = False
        for arg in node.args:
            has_done_something = self._rewrite_SSAValue_type(arg)
        return RewriteResult(has_done_something=has_done_something)

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        has_done_something = False
        for result in node.results:
            has_done_something = self._rewrite_SSAValue_type(result)
        return RewriteResult(has_done_something=has_done_something)

    def _rewrite_SSAValue_type(self, value: ir.SSAValue):
        # NOTE: cannot use issubseteq here because type can be Bottom
        if isinstance(value.type, ir.types.Generic) and issubclass(
            value.type.body.typ, list
        ):
            value.type = IListType[value.type.vars[0], ir.types.Any]
            return True
        elif isinstance(value.type, ir.types.PyClass) and issubclass(
            value.type.typ, list
        ):
            value.type = IListType[ir.types.Any, ir.types.Any]
            return True
        return False
