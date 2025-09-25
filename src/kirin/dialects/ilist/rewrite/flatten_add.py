from kirin import ir
from kirin.analysis import const
from kirin.dialects import py, ilist
from kirin.rewrite.abc import RewriteRule, RewriteResult


class FlattenAdd(RewriteRule):

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not (
            isinstance(node, py.binop.Add)
            and node.lhs.type.is_subseteq(ilist.IListType)
            and node.rhs.type.is_subseteq(ilist.IListType)
        ):
            return RewriteResult()

        # check if we are adding two ilist.New objects
        new_data = ()

        # lhs:
        if isinstance((lhs := node.lhs).owner, ilist.New):
            new_data += lhs.owner.values
        elif (
            not isinstance(const_lhs := lhs.hints.get("const"), const.Value)
            or len(const_lhs.data) > 0
        ):
            return RewriteResult()

        # rhs:
        if isinstance((rhs := node.rhs).owner, ilist.New):
            new_data += rhs.owner.values
        elif (
            not isinstance(const_rhs := rhs.hints.get("const"), const.Value)
            or len(const_rhs.data) > 0
        ):
            return RewriteResult()

        node.replace_by(ilist.New(values=new_data))

        return RewriteResult(has_done_something=True)
