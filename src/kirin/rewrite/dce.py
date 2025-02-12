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

    def is_pure(self, node: ir.Statement):
        if node.has_trait(ir.Pure):
            return True

        if (trait := node.get_trait(ir.MaybePure)) and trait.is_pure(node):
            return True
        return False
