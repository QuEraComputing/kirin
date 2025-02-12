from dataclasses import dataclass

from kirin.rewrite import Walk, Chain, Fixpoint
from kirin.analysis import const
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.rewrite.dce import DeadCodeElimination
from kirin.rewrite.fold import ConstantFold
from kirin.ir.nodes.base import IRNode
from kirin.rewrite.inline import Inline
from kirin.rewrite.getitem import InlineGetItem
from kirin.rewrite.getfield import InlineGetField
from kirin.rewrite.compactify import CFGCompactify
from kirin.rewrite.wrap_const import WrapConst
from kirin.rewrite.call2invoke import Call2Invoke


@dataclass
class Fold(RewriteRule):
    rule: RewriteRule

    def __init__(self, frame: const.Frame):
        rule = Fixpoint(
            Chain(
                Walk(WrapConst(frame)),
                Walk(Inline(lambda _: True)),
                Walk(ConstantFold()),
                Walk(Call2Invoke()),
                Fixpoint(
                    Walk(
                        Chain(
                            InlineGetItem(),
                            InlineGetField(),
                            DeadCodeElimination(),
                        )
                    )
                ),
                Walk(CFGCompactify()),
            )
        )
        self.rule = rule

    def rewrite(self, node: IRNode) -> RewriteResult:
        return self.rule.rewrite(node)
