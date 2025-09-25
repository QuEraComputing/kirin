from kirin import ir
from kirin.dialects import py, ilist
from kirin.rewrite.abc import RewriteRule, RewriteResult


class FlattenAdd(RewriteRule):

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, py.binop.Add):
            return RewriteResult()

        # check if we are adding two ilist.New objects
        new_data = ()

        # lhs:
        if not isinstance(node.lhs.owner, ilist.New):
            if not (
                isinstance(node.lhs.owner, py.Constant)
                and isinstance(
                    const_ilist := node.lhs.owner.value.unwrap(), ilist.IList
                )
                and len(const_ilist.data) == 0
            ):
                return RewriteResult()

        else:
            new_data += node.lhs.owner.values

        # rhs:
        if not isinstance(node.rhs.owner, ilist.New):
            if not (
                isinstance(node.rhs.owner, py.Constant)
                and isinstance(
                    const_ilist := node.rhs.owner.value.unwrap(), ilist.IList
                )
                and len(const_ilist.data) == 0
            ):
                return RewriteResult()

        else:
            new_data += node.rhs.owner.values

        new_stmt = ilist.New(values=new_data)
        node.replace_by(new_stmt)

        return RewriteResult(
            has_done_something=True,
        )
