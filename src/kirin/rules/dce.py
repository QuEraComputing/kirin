from dataclasses import dataclass, field

from kirin import ir
from kirin.analysis import const
from kirin.dialects import func
from kirin.rewrite import RewriteResult, RewriteRule


@dataclass
class DeadCodeElimination(RewriteRule):
    results: dict[ir.SSAValue, const.JointResult] = field(default_factory=dict)

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
                value = self.results.get(result, None)
                if value is None:
                    return False

                if value.purity is not const.Pure():
                    return False
            return True

        return False
