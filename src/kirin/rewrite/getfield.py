from dataclasses import dataclass

from kirin import ir
from kirin.dialects import func
from kirin.rewrite.abc import RewriteRule, RewriteResult


@dataclass
class InlineGetField(RewriteRule):

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, func.GetField):
            return RewriteResult()

        if not isinstance(node.obj.owner, func.Lambda):
            return RewriteResult()

        original = node.obj.owner.captured[node.field]
        node.result.replace_by(original)
        node.delete()
        return RewriteResult(has_done_something=True)
