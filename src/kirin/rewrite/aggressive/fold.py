from typing import Any
from dataclasses import dataclass

from kirin import ir, analysis
from kirin.rewrite import Walk, Chain, Fixpoint
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.rewrite.dce import DeadCodeElimination
from kirin.rewrite.fold import ConstantFold
from kirin.ir.nodes.base import IRNode
from kirin.rewrite.inline import Inline
from kirin.rewrite.getitem import InlineGetItem
from kirin.rewrite.getfield import InlineGetField
from kirin.rewrite.wrap_const import WrapConst
from kirin.rewrite.call2invoke import Call2Invoke
from kirin.rewrite.cfg_compactify import CFGCompactify


@dataclass
class Fold(RewriteRule):
    rule: RewriteRule

    def __init__(self, cfg: analysis.CFG, results: dict[ir.SSAValue, Any]):
        rule = Fixpoint(
            Chain(
                [
                    Walk(Inline(lambda _: True)),
                    Walk(ConstantFold(results)),
                    Walk(Call2Invoke(results)),
                    Walk(WrapConst(results)),
                    Fixpoint(
                        Walk(
                            Chain(
                                [
                                    InlineGetItem(results),
                                    InlineGetField(),
                                    DeadCodeElimination(results),
                                ]
                            )
                        )
                    ),
                    Fixpoint(CFGCompactify(cfg)),
                ]
            )
        )
        self.rule = rule

    def rewrite(self, node: IRNode) -> RewriteResult:
        return self.rule.rewrite(node)
