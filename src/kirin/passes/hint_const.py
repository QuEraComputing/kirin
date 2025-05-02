from dataclasses import dataclass

from kirin.ir import Method
from kirin.rewrite import Walk, WrapConst
from kirin.analysis import const
from kirin.passes.abc import Pass
from kirin.rewrite.abc import RewriteResult


@dataclass
class HintConst(Pass):

    def unsafe_run(self, mt: Method) -> RewriteResult:
        constprop = const.Propagate(self.dialects)
        if self.no_raise:
            frame, _ = constprop.run_no_raise(mt)
        else:
            frame, _ = constprop.run(mt)
        return Walk(WrapConst(frame)).rewrite(mt.code)
