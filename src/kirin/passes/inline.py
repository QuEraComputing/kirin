from typing import Callable
from dataclasses import field, dataclass

from kirin import ir
from kirin.passes import Pass
from kirin.rewrite import Walk, Fixpoint, RewriteResult
from kirin.rewrite.dce import DeadCodeElimination
from kirin.rewrite.inline import Inline
from kirin.rewrite.cfg_compactify import CFGCompactify


def aggresive(x: ir.IRNode) -> bool:
    return True


@dataclass
class InlinePass(Pass):
    herustic: Callable[[ir.IRNode], bool] = field(default=aggresive)

    def unsafe_run(self, mt: ir.Method) -> RewriteResult:

        result = Walk(Inline(heuristic=self.herustic)).rewrite(mt.code)

        if (trait := mt.code.get_trait(ir.SSACFGRegion)) is not None:
            compactify = Fixpoint(CFGCompactify(trait.get_graph(mt.callable_region)))
            result = compactify.rewrite(mt.code).join(result)

        # dce
        dce = DeadCodeElimination()
        return Fixpoint(Walk(dce)).rewrite(mt.code).join(result)
