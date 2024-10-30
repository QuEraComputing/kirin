from dataclasses import dataclass

from kirin import ir
from kirin.dialects.py import stmts
from kirin.rewrite import RewriteResult, RewriteRule


@dataclass
class InlineAlias(RewriteRule):

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, stmts.Alias):
            return RewriteResult()

        node.result.replace_by(node.value)
        return RewriteResult(has_done_something=True)
