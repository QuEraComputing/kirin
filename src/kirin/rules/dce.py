from dataclasses import dataclass, field

from kirin import ir
from kirin.analysis.dataflow.constprop import ConstPropLattice, NotPure
from kirin.dialects import func
from kirin.rewrite import RewriteResult, RewriteRule


@dataclass
class DeadCodeElimination(RewriteRule):
    results: dict[ir.SSAValue, ConstPropLattice] = field(default_factory=dict)

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
            return not any(
                isinstance(self.results.get(result, NotPure()), NotPure)
                for result in node.results
            )
        return False
