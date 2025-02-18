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
        frame, _ = constprop.run_analysis(mt)
        result = Walk(WrapConst(frame)).rewrite(mt.code)
        result = (
            Fixpoint(
                Walk(
                    Chain(
                        ConstantFold(),
                        InlineGetItem(),
                        Call2Invoke(),
                        DeadCodeElimination(),
                    )
                )
            )
            .rewrite(mt.code)
            .join(result)
        )

        if mt.code.has_trait(SSACFGRegion):
            result = Walk(CFGCompactify()).rewrite(mt.code).join(result)

        return Fixpoint(Walk(DeadCodeElimination())).rewrite(mt.code).join(result)
