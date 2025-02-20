from dataclasses import dataclass

from kirin import ir
from kirin.analysis import const
from kirin.dialects import py
from kirin.rewrite.abc import RewriteRule, RewriteResult


@dataclass
class InlineGetItem(RewriteRule):

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, py.indexing.GetItem):
            return RewriteResult()

        if not isinstance(node.obj.owner, py.tuple.New):
            return RewriteResult()

        if not isinstance(index_value := node.index.hints.get("const"), const.Value):
            return RewriteResult()

        stmt = node.obj.owner
        index = index_value.data
        if isinstance(index, int) and (
            0 <= index < len(stmt.args) or -len(stmt.args) <= index < 0
        ):
            node.result.replace_by(stmt.args[index])
            return RewriteResult(has_done_something=True)
        elif isinstance(index, slice):
            start, stop, step = index.indices(len(stmt.args))
            new_tuple = py.tuple.New(
                tuple(stmt.args[start:stop:step]),
            )
            node.replace_by(new_tuple)
            return RewriteResult(has_done_something=True)
        else:
            return RewriteResult()
