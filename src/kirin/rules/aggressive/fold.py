from dataclasses import dataclass
from typing import Any

from kirin import analysis, ir
from kirin.ir.nodes.base import IRNode
from kirin.rewrite import Chain, Fixpoint, RewriteResult, RewriteRule, Walk
from kirin.rules.call2invoke import Call2Invoke
from kirin.rules.cfg_compactify import CFGCompactify
from kirin.rules.dce import DeadCodeElimination
from kirin.rules.fold import ConstantFold
from kirin.rules.getfield import InlineGetField
from kirin.rules.getitem import InlineGetItem
from kirin.rules.inline import Inline


@dataclass
class Fold(RewriteRule):
    rule: RewriteRule

    def __init__(self, cfg: analysis.CFG, results: dict[ir.SSAValue, Any]):
        rule = Fixpoint(
            Chain(
                [
                    Walk(Inline(lambda _: True)),
                    ConstantFold(results),
                    Call2Invoke(results),
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
