from kirin import ir
from kirin.rewrite import abc
from kirin.analysis import const
from kirin.dialects import py

from ..stmts import New


class InlineGetItem(abc.RewriteRule):
    """Rewrite rule to inline GetItem statements for IList.

    For example if we have an `ilist.New` statement with a list of items,
    and we can infer that the index used in `py.GetItem` is constant and within bounds,
    we replace the `py.GetItem` with the ssa value in the list when the index is an integer
    or with a new `ilist.New` statement containing the sliced items when the index is a slice.

    """

    def rewrite_Statement(self, node: ir.Statement) -> abc.RewriteResult:
        if not isinstance(node, py.GetItem) or not isinstance(
            stmt := node.obj.owner, New
        ):
            return abc.RewriteResult()

        if not isinstance(index_const := node.index.hints.get("const"), const.Value):
            return abc.RewriteResult()

        index = index_const.data
        if isinstance(index, int) and (
            0 <= index < len(stmt.args) or -len(stmt.args) <= index < 0
        ):
            node.result.replace_by(stmt.args[index])
            return abc.RewriteResult(has_done_something=True)
        elif isinstance(index, slice):
            start, stop, step = index.indices(len(stmt.args))
            new_tuple = New(
                tuple(stmt.args[start:stop:step]),
            )
            node.replace_by(new_tuple)
            return abc.RewriteResult(has_done_something=True)
        else:
            return abc.RewriteResult()
