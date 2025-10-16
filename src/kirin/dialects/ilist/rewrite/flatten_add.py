from kirin import ir, types
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
        lhs = node.lhs
        rhs = node.rhs

        if (
            (lhs_parent := lhs.owner.parent) is None
            or (rhs_parent := rhs.owner.parent) is None
            or lhs_parent is not rhs_parent
        ):
            # do not flatten across different blocks/regions
            return RewriteResult()

        if isinstance(lhs.owner, ilist.New):
            new_data += lhs.owner.values
        elif (
            not isinstance(const_lhs := lhs.hints.get("const"), const.Value)
            or len(const_lhs.data) > 0
        ):
            return RewriteResult()

        # rhs:
        if isinstance(rhs.owner, ilist.New):
            new_data += rhs.owner.values
        elif (
            not isinstance(const_rhs := rhs.hints.get("const"), const.Value)
            or len(const_rhs.data) > 0
        ):
            return RewriteResult()

        assert isinstance(rhs_type := rhs.type, types.Generic), "Impossible"
        assert isinstance(lhs_type := lhs.type, types.Generic), "Impossible"

        lhs_elem_type = lhs_type.vars[0]
        rhs_elem_type = rhs_type.vars[0]

        result_elem_type = lhs_elem_type.join(rhs_elem_type)
        node.replace_by(ilist.New(values=new_data, elem_type=result_elem_type))

        return RewriteResult(has_done_something=True)
