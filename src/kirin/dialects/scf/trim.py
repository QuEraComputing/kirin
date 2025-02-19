from kirin import ir
from kirin.rewrite.abc import RewriteRule
from kirin.rewrite.result import RewriteResult

from .stmts import For, Yield, IfElse


class UnusedYield(RewriteRule):
    """Trim unused results from `For` and `IfElse` statements."""

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, (For, IfElse)):
            return RewriteResult()

        any_unused = False
        uses: list[int] = []
        results: list[ir.ResultValue] = []
        for idx, result in enumerate(node.results):
            if result.uses:
                uses.append(idx)
                results.append(result)
            else:
                any_unused = True

        if not any_unused:
            return RewriteResult()

        node._results = results
        for region in node.regions:
            for block in region.blocks:
                if not isinstance(block.last_stmt, Yield):
                    continue

                block.last_stmt.args = [block.last_stmt.args[idx] for idx in uses]

        return RewriteResult(has_done_something=True)
