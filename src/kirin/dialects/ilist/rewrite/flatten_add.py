from kirin import ir
from kirin.dialects import py, ilist
from kirin.rewrite.abc import RewriteRule, RewriteResult


class FlattenAddOpIList(RewriteRule):

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if (
            not isinstance(node, py.binop.Add)
            or not isinstance(lhs := node.lhs.owner, ilist.New)
            or not isinstance(rhs := node.rhs.owner, ilist.New)
        ):
            return RewriteResult()

        node.replace_by(ilist.New(values=lhs.values + rhs.values))

        return RewriteResult(has_done_something=True)
