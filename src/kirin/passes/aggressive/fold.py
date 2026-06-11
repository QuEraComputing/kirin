from typing import Callable
from dataclasses import field, dataclass

from kirin import ir
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


def _inline_everything(stmt: ir.Statement) -> bool:
    """Default inline heuristic: inline every callee (historical behavior)."""
    return True


@dataclass
class Fold(Pass):
    inline_heuristic: Callable[[ir.Statement], bool] = field(
        default=_inline_everything, kw_only=True
    )
    """Predicate over a callee's code statement deciding whether to inline it.

    Defaults to inlining everything. Provide a custom predicate to fold
    everything *except* selected callees -- e.g. to keep a per-call kernel as a
    swappable hole::

        aggressive.Fold(
            dialects, inline_heuristic=lambda code: code.sym_name != "science"
        )
    """
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
        result = Walk(Inline(self.inline_heuristic)).rewrite(mt.code).join(result)
        result = Fixpoint(CFGCompactify()).rewrite(mt.code).join(result)
        return result
