from dataclasses import dataclass

from stmts import RandomBranch

from kirin.dialects import cf
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.ir.nodes.stmt import Statement


@dataclass
class RandomWalkBranch(RewriteRule):

    def rewrite_Statement(self, node: Statement) -> RewriteResult:
        if not isinstance(node, cf.ConditionalBranch):
            return RewriteResult()
        node.replace_by(
            RandomBranch(
                cond=node.cond,
                then_arguments=node.then_arguments,
                then_successor=node.then_successor,
                else_arguments=node.else_arguments,
                else_successor=node.else_successor,
            )
        )
        return RewriteResult(has_done_something=True)
