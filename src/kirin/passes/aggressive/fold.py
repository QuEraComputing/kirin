from dataclasses import dataclass

from kirin.analysis import CFG, const
from kirin.ir.method import Method
from kirin.passes import Pass
from kirin.rewrite import RewriteResult
from kirin.rules import aggressive


@dataclass
class Fold(Pass):

    def unsafe_run(self, mt: Method) -> RewriteResult:
        cfg = CFG(mt.callable_region)
        constprop = const.Propagate(self.dialects)
        constprop.eval(mt, tuple(const.JointResult.top() for _ in mt.args))
        return aggressive.Fold(cfg, constprop.results).rewrite(mt.code)
