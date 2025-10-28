from typing import cast

from kirin import ir, types
from kirin.analysis import const
from kirin.dialects import py
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.dialects.ilist.stmts import IListType

from .._dialect import dialect


@dialect.post_inference
class HintLen(RewriteRule):

    def _get_collection_len(self, collection: ir.SSAValue):
        coll_type = collection.type

        if not isinstance(coll_type, types.Generic):
            return None

        if (
            coll_type.is_subseteq(IListType)
            and isinstance(coll_type.vars[1], types.Literal)
            and isinstance(coll_type.vars[1].data, int)
        ):
            return coll_type.vars[1].data
        else:
            return None

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, py.Len):
            return RewriteResult()

        if (coll_len := self._get_collection_len(node.value)) is None:
            return RewriteResult()

        existing_hint = cast(const.Result | None, node.result.hints.get("const"))
        new_hint = const.Value(coll_len)

        if existing_hint is not None and new_hint.is_equal(existing_hint):
            return RewriteResult()

        node.result.hints["const"] = new_hint
        return RewriteResult(has_done_something=True)
