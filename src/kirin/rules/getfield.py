from dataclasses import dataclass

from kirin import ir
from kirin.rewrite import RewriteRule, RewriteResult
from kirin.dialects import func


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
