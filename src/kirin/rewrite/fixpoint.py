from dataclasses import dataclass

from kirin.ir import IRNode
from kirin.rewrite.abc import RewriteRule, RewriteResult


@dataclass
class Fixpoint(RewriteRule):
    """Apply a rewrite rule until a fixpoint is reached.

    The rewrite rule is applied to the node until the rewrite rule does not do anything.

    ### Parameters
    - `map`: The rewrite rule to apply.
    - `max_iter`: The maximum number of iterations to apply the rewrite rule. Default is 32.
    """

    rule: RewriteRule
    max_iter: int = 32

    def rewrite(self, node: IRNode) -> RewriteResult:
        has_done_something = False
        for _ in range(self.max_iter):
            result = self.rule.rewrite(node)
            if result.terminated:
                return result

            if result.has_done_something:
                has_done_something = True
            else:
                return RewriteResult(has_done_something=has_done_something)

        return RewriteResult(exceeded_max_iter=True)
