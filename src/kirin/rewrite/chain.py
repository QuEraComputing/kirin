from typing import Iterable
from dataclasses import dataclass

from kirin.ir import IRNode
from kirin.rewrite.abc import RewriteRule
from kirin.rewrite.result import RewriteResult


@dataclass
class Chain(RewriteRule):
    """Chain multiple rewrites together.

    The chain will apply each rewrite in order until one of the rewrites terminates.
    """

    rules: list[RewriteRule]

    def __init__(self, rule: RewriteRule | Iterable[RewriteRule], *others: RewriteRule):
        if isinstance(rule, RewriteRule):
            self.rules = [rule, *others]
        else:
            assert (
                others == ()
            ), "Cannot pass multiple positional arguments if the first argument is an iterable"
            self.rules = list(rule)

    def rewrite(self, node: IRNode) -> RewriteResult:
        has_done_something = False
        for rule in self.rules:
            result = rule.rewrite(node)
            if result.terminated:
                return result

            if result.has_done_something:
                has_done_something = True
        return RewriteResult(has_done_something=has_done_something)

    def __repr__(self):
        return " -> ".join(map(str, self.rules))
