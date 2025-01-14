from dataclasses import dataclass

from kirin.ir import Method, SSACFGRegion
from kirin.rewrite import Walk, Chain, Fixpoint, RewriteResult
from kirin.analysis import const
from kirin.passes.abc import Pass
from kirin.rewrite.dce import DeadCodeElimination
from kirin.rewrite.fold import ConstantFold
from kirin.rewrite.getitem import InlineGetItem
from kirin.rewrite.wrap_const import WrapConst
from kirin.rewrite.call2invoke import Call2Invoke
from kirin.rewrite.cfg_compactify import CFGCompactify


@dataclass
class Fold(Pass):

    def unsafe_run(self, mt: Method) -> RewriteResult:
        constprop = const.Propagate(self.dialects)
        constprop.eval(mt, tuple(const.JointResult.top() for _ in mt.args))
        result = Fixpoint(
            Walk(
                Chain(
                    [
                        ConstantFold(constprop.results),
                        WrapConst(constprop.results),
                        InlineGetItem(constprop.results),
                        Call2Invoke(constprop.results),
                        DeadCodeElimination(constprop.results),
                    ]
                )
            )
        ).rewrite(mt.code)

        if (trait := mt.code.get_trait(SSACFGRegion)) is not None:
            compactify = Fixpoint(CFGCompactify(trait.get_graph(mt.callable_region)))
            result = compactify.rewrite(mt.code).join(result)

        return (
            Fixpoint(Walk(DeadCodeElimination(constprop.results)))
            .rewrite(mt.code)
            .join(result)
        )
