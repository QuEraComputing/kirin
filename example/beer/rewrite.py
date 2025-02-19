from dataclasses import dataclass

from stmts import Nap, Eat, NewFood, RandomBranch

from kirin import ir
from kirin.dialects import cf
from kirin.rewrite.abc import RewriteRule, RewriteResult


@dataclass
class RandomWalkBranch(RewriteRule):

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
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


@dataclass
class NewFoodAndNap(RewriteRule):
    # sometimes someone is hungry and needs a nap
    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, Eat):
            return RewriteResult()

        # 1. create new stmts:
        new_food_stmt = NewFood(type="burger")
        nap_stmt = Nap()

        # 2. put them in the ir
        new_food_stmt.insert_after(node)
        nap_stmt.insert_after(new_food_stmt)

        return RewriteResult(has_done_something=True)
