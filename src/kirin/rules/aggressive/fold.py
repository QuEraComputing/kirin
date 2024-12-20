from typing import Any
from dataclasses import dataclass

from kirin import ir, analysis
from kirin.rewrite import Walk, Chain, Fixpoint, RewriteRule, RewriteResult
from kirin.rules.dce import DeadCodeElimination
from kirin.rules.fold import ConstantFold
from kirin.rules.inline import Inline
from kirin.ir.nodes.base import IRNode
from kirin.rules.getitem import InlineGetItem
from kirin.rules.getfield import InlineGetField
from kirin.rules.call2invoke import Call2Invoke
from kirin.rules.cfg_compactify import CFGCompactify


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
