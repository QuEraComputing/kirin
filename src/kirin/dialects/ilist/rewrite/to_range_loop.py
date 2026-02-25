from kirin import types
from kirin.dialects import py, scf, ilist
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.ir.nodes.stmt import Statement


class ToRangeFor(RewriteRule):
    """Rewrite for-loops over `IList` iterables into for-loops over the range of their length.

    For example, rewrites:

    ```python
    for ele in ilist_values:
        ...

    ```

    to

    ```python
    for i in range(len(ilist_values)):
        ele = ilist_values[i]
        ...

    ```
    """

    def rewrite_Statement(self, node: Statement) -> RewriteResult:
        if not (
            isinstance(node, scf.For)
            and (iterable := node.iterable).type.is_subseteq(ilist.IListType)
            and not iterable.type.is_structurally_equal(types.Bottom)
        ):
            return RewriteResult()

        body_block = node.body.blocks[0]

        ele_arg = body_block.args[0]
        index = body_block.args.insert_from(0, types.Int)
        (ele_getitem := py.GetItem(iterable, index)).insert_before(
            body_block.first_stmt
        )
        ele_getitem.result.name = ele_arg.name
        ele_arg.replace_by(ele_getitem.result)
        body_block.args.delete(ele_arg)

        (len_stmt := py.Len(node.iterable)).insert_before(node)
        (zero := py.Constant(0)).insert_before(node)
        (one := py.Constant(1)).insert_before(node)
        (
            range_stmt := ilist.stmts.Range(zero.result, len_stmt.result, one.result)
        ).insert_before(node)

        node.iterable = range_stmt.result

        return RewriteResult(has_done_something=True)
