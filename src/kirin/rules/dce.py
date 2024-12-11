from dataclasses import dataclass, field

from kirin import ir
from kirin.analysis import JointResult, purity
from kirin.dialects import func
from kirin.rewrite import RewriteResult, RewriteRule


@dataclass
class DeadCodeElimination(RewriteRule):
    results: dict[ir.SSAValue, JointResult] = field(default_factory=dict)

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

        if isinstance(node, func.Invoke):
            for result in node.results:
                if not isinstance(self.results.get(result, None), purity.Pure):
                    return False
            return True

        return False
