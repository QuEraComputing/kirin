from dataclasses import dataclass

from stmts import Puke, Drink, NewBeer, RandomBranch

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


@dataclass
class NewBeerAndPukeOnDrink(RewriteRule):
    # sometimes someone get drunk, so they keep getting new beer and puke after they drink
    def rewrite_Statement(self, node: Statement) -> RewriteResult:
        if not isinstance(node, Drink):
            return RewriteResult()

        # 1. create new stmts:
        new_beer_stmt = NewBeer(brand="saporo")
        puke_stmt = Puke()

        # 2. put them in the ir
        new_beer_stmt.insert_after(node)
        puke_stmt.insert_after(new_beer_stmt)

        return RewriteResult(has_done_something=True)
