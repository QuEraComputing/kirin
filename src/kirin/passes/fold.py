from dataclasses import field, dataclass

from kirin.ir import Method, SSACFGRegion
from kirin.rewrite import (
    Walk,
    Chain,
    Fixpoint,
    Call2Invoke,
    ConstantFold,
    CFGCompactify,
    InlineGetItem,
    DeadCodeElimination,
)
from kirin.passes.abc import Pass
from kirin.rewrite.abc import RewriteResult

from .hint_const import HintConst


@dataclass
class Fold(Pass):
    hint_const: HintConst = field(init=False)

    def __post_init__(self):
        self.hint_const = HintConst(self.dialects)
        self.hint_const.no_raise = self.no_raise

    def unsafe_run(self, mt: Method) -> RewriteResult:
        result = self.hint_const.unsafe_run(mt)
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
