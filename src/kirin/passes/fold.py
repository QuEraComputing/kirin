from dataclasses import dataclass

from kirin.ir import Method, SSACFGRegion
from kirin.rewrite import (
    Walk,
    Chain,
    Fixpoint,
    WrapConst,
    Call2Invoke,
    ConstantFold,
    CFGCompactify,
    InlineGetItem,
    DeadCodeElimination,
)
from kirin.analysis import const
from kirin.passes.abc import Pass
from kirin.rewrite.abc import RewriteResult


@dataclass
class Fold(Pass):

    def unsafe_run(self, mt: Method) -> RewriteResult:
        constprop = const.Propagate(self.dialects)
        constprop_results, _ = constprop.run_analysis(mt)
        result = Fixpoint(
            Walk(
                Chain(
                    ConstantFold(constprop_results),
                    WrapConst(constprop_results),
                    InlineGetItem(constprop_results),
                    Call2Invoke(constprop_results),
                    DeadCodeElimination(constprop_results),
                )
            )
        ).rewrite(mt.code)

        if mt.code.has_trait(SSACFGRegion):
            result = Walk(CFGCompactify()).rewrite(mt.code).join(result)

        return (
            Fixpoint(Walk(DeadCodeElimination(constprop_results)))
            .rewrite(mt.code)
            .join(result)
        )
