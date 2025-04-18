from dataclasses import field, dataclass

from kirin.passes import Pass
from kirin.rewrite import (
    Walk,
    Chain,
    Inline,
    Fixpoint,
    Call2Invoke,
    ConstantFold,
    CFGCompactify,
    InlineGetItem,
    InlineGetField,
    DeadCodeElimination,
)
from kirin.ir.method import Method
from kirin.rewrite.abc import RewriteResult
from kirin.passes.hint_const import HintConst


@dataclass
class Fold(Pass):
    hint_const: HintConst = field(init=False)

    def __post_init__(self):
        self.hint_const = HintConst(self.dialects)
        self.hint_const.no_raise = self.no_raise

    def unsafe_run(self, mt: Method) -> RewriteResult:
        result = self.hint_const.unsafe_run(mt)
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
