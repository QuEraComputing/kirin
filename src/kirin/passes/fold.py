from dataclasses import dataclass

from kirin.analysis.dataflow.constprop import ConstProp, NotConst
from kirin.analysis.dataflow.reachable import EmptyLattice, ReachableAnalysis
from kirin.ir import Method
from kirin.passes.abc import Pass
from kirin.rewrite import Fixpoint, Walk
from kirin.rules.dce import DeadCodeElimination
from kirin.rules.fold import ConstantFold


@dataclass
class Fold(Pass):

    def __call__(self, mt: Method) -> None:
        constprop = ConstProp(self.dialects)
        constprop.eval(mt, tuple(NotConst() for _ in mt.args))
        Fixpoint(Walk(ConstantFold(results=constprop.results))).rewrite(mt.code)

        reachability = ReachableAnalysis(self.dialects)
        reachability.eval(mt, tuple(EmptyLattice() for _ in mt.args))
        dce = DeadCodeElimination(cfg=reachability.visited)
        Walk(dce).rewrite(mt.code)
