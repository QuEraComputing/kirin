from dataclasses import field, dataclass

from kirin.passes import Pass
from kirin.rewrite import (
    Walk,
    Chain,
    Inline,
    Fixpoint,
    WrapConst,
    Call2Invoke,
    ConstantFold,
    CFGCompactify,
    InlineGetItem,
    InlineGetField,
    DeadCodeElimination,
)
from kirin.analysis import const
from kirin.ir.method import Method
from kirin.rewrite.abc import RewriteResult


@dataclass
class Fold(Pass):
    constprop: const.Propagate = field(init=False)

    def __post_init__(self):
        self.constprop = const.Propagate(self.dialects)

    def unsafe_run(self, mt: Method) -> RewriteResult:
        result = RewriteResult()
        frame, _ = self.constprop.run_analysis(mt)
        result = Walk(WrapConst(frame)).rewrite(mt.code).join(result)
        rule = Chain(
            ConstantFold(),
            Call2Invoke(),
            InlineGetField(),
            InlineGetItem(),
            DeadCodeElimination(),
        )
        result = Fixpoint(Walk(rule)).rewrite(mt.code).join(result)
        result = Walk(Inline(lambda _: True)).rewrite(mt.code).join(result)
        result = Fixpoint(CFGCompactify()).rewrite(mt.code).join(result)
        return result
