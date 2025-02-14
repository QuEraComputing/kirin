from dataclasses import dataclass

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
    max_iter: int = 10

    def unsafe_run(self, mt: Method) -> RewriteResult:
        result = RewriteResult()
        constprop = const.Propagate(self.dialects)
        for _ in range(self.max_iter):
            frame, _ = constprop.run_analysis(mt)
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
            if result.has_done_something is False:
                return RewriteResult(
                    has_done_something=True,
                    terminated=result.terminated,
                    exceeded_max_iter=result.exceeded_max_iter,
                )
            if result.terminated:
                return result
        return RewriteResult(
            has_done_something=True, terminated=False, exceeded_max_iter=True
        )
