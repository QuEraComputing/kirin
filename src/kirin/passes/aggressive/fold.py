from dataclasses import dataclass

from kirin.passes import Pass
from kirin.rewrite import RewriteResult, aggressive
from kirin.analysis import CFG, const
from kirin.ir.method import Method


@dataclass
class Fold(Pass):

    def unsafe_run(self, mt: Method) -> RewriteResult:
        cfg = CFG(mt.callable_region)
        constprop = const.Propagate(self.dialects)
        constprop.eval(mt, tuple(const.JointResult.top() for _ in mt.args))
        return aggressive.Fold(cfg, constprop.results).rewrite(mt.code)
