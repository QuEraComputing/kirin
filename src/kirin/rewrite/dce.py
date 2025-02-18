from dataclasses import dataclass

from kirin import ir
from kirin.rewrite.abc import RewriteRule, RewriteResult


@dataclass
class DeadCodeElimination(RewriteRule):

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if self.is_pure(node):
            for result in node._results:
                if result.uses:
                    return RewriteResult()

            node.delete()
            return RewriteResult(has_done_something=True)

        return RewriteResult()
