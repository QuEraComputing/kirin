from dataclasses import dataclass

from kirin.analysis.dataflow.constprop import ConstProp, NotConst
from kirin.ir import Method, SSACFGRegion
from kirin.passes.abc import Pass
from kirin.rewrite import Fixpoint, Walk
from kirin.rules.cfg_compatify import CFGCompactify
from kirin.rules.dce import DeadCodeElimination
from kirin.rules.fold import ConstantFold


@dataclass
class Fold(Pass):

    def __call__(self, mt: Method) -> None:
        constprop = ConstProp(self.dialects)
        constprop.eval(mt, tuple(NotConst() for _ in mt.args))
        Fixpoint(Walk(ConstantFold(results=constprop.results))).rewrite(mt.code)
        dce = DeadCodeElimination()
        Fixpoint(Walk(dce)).rewrite(mt.code)

        if (trait := mt.code.get_trait(SSACFGRegion)) is not None:
            compactify = Fixpoint(CFGCompactify(trait.get_graph(mt.callable_region)))
            compactify.rewrite(mt.code)
