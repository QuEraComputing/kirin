from dataclasses import dataclass

from kirin.ir import Pure, Block, Statement
from kirin.rewrite.abc import RewriteRule, RewriteResult


@dataclass
class CommonSubexpressionElimination(RewriteRule):

    def rewrite_Block(self, node: Block) -> RewriteResult:
        seen: dict[int, Statement] = {}

        for stmt in node.stmts:
            if not stmt.has_trait(Pure):
                continue

            if stmt.regions:
                continue

            hash_value = hash(
                (type(stmt),)
                + tuple(stmt.args)
                + tuple(stmt.attributes.values())
                + tuple(stmt.successors)
                + tuple(stmt.regions)
            )
            if hash_value in seen:
                old_stmt = seen[hash_value]
                for result in stmt._results:
                    result.replace_by(old_stmt._results[0])
                stmt.delete()
                return RewriteResult(has_done_something=True)
            else:
                seen[hash_value] = stmt
        return RewriteResult()

    def rewrite_Statement(self, node: Statement) -> RewriteResult:
        if not node.regions:
            return RewriteResult()

        has_done_something = False
        for region in node.regions:
            for block in region.blocks:
                result = self.rewrite_Block(block)
                if result.has_done_something:
                    has_done_something = True

        return RewriteResult(has_done_something=has_done_something)
