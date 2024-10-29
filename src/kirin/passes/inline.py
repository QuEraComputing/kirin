from dataclasses import dataclass

from kirin import ir
from kirin.passes import Pass
from kirin.rewrite import Fixpoint, Walk
from kirin.rules.cfg_compatify import CFGCompactify
from kirin.rules.dce import DeadCodeElimination
from kirin.rules.inline import Inline


@dataclass
class InlinePass(Pass):

    def unsafe_run(self, mt: ir.Method) -> None:

        Walk(Inline(heuristic=lambda x: True)).rewrite(mt.code)

        if (trait := mt.code.get_trait(ir.SSACFGRegion)) is not None:
            compactify = Fixpoint(CFGCompactify(trait.get_graph(mt.callable_region)))
            compactify.rewrite(mt.code)

        # dce
        dce = DeadCodeElimination()
        Fixpoint(Walk(dce)).rewrite(mt.code)
