from dataclasses import dataclass

from kirin.ir import Pure, Statement
from kirin.rewrite import RewriteResult, RewriteRule


@dataclass
class DeadCodeElimination(RewriteRule):

    def rewrite_Statement(self, node: Statement) -> RewriteResult:
        if not node.has_trait(Pure):
            return RewriteResult()

        for result in node._results:
            if result.uses:
                return RewriteResult()

        node.delete()
        return RewriteResult(has_done_something=True)
