from dataclasses import dataclass

from kirin.dialects.py import stmts as py_stmts
from kirin.ir import Block, Pure, Statement
from kirin.rewrite import RewriteResult, RewriteRule


@dataclass
class CommonSubexpressionElimination(RewriteRule):

    def rewrite_Block(self, node: Block) -> RewriteResult:
        seen: dict[int, Statement] = {}

        for stmt in node.stmts:
            if not stmt.has_trait(Pure):
                continue

            if stmt.regions:
                continue

            # the result of a statement only depends on its arguments now
            if isinstance(stmt, py_stmts.Constant):
                hash_value = hash((type(stmt), stmt.value))
            else:
                hash_value = hash((type(stmt),) + tuple(stmt.args))

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
