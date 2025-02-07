from dataclasses import dataclass

from kirin.passes import Pass
from kirin.rewrite import aggressive
from kirin.analysis import const
from kirin.ir.method import Method
from kirin.rewrite.abc import RewriteResult


@dataclass
class Fold(Pass):

    def unsafe_run(self, mt: Method) -> RewriteResult:
        constprop = const.Propagate(self.dialects)
        constprop_results, _ = constprop.run_analysis(mt)
        return aggressive.Fold(constprop_results).rewrite(mt.code)
