from kirin import ir
from kirin.rewrite.abc import RewriteRule, RewriteResult

from .stmts import For, Yield, IfElse
from ._dialect import dialect


@dialect.canonicalize
class UnusedYield(RewriteRule):
    """Trim unused results from `For` and `IfElse` statements."""

    def scan_unused(self, node: ir.Statement):
        any_unused = False
        uses: list[int] = []
        results: list[ir.ResultValue] = []
        for idx, result in enumerate(node.results):
            if result.uses:
                uses.append(idx)
                results.append(result)
            else:
                any_unused = True
        return any_unused, set(uses), results

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, (For, IfElse)):
            return RewriteResult()

        any_unused, uses, results = self.scan_unused(node)
        if not any_unused:
            return RewriteResult()

        node._results = results
        for region in node.regions:
            for block in region.blocks:
                if not isinstance(block.last_stmt, Yield):
                    continue

                block.last_stmt.args = [block.last_stmt.args[idx] for idx in uses]

        if isinstance(node, For):
            for idx, block_arg in enumerate(node.body.blocks[0].args[1:]):
                if idx not in uses:
                    block_arg.replace_by(node.initializers[idx])
                    block_arg.delete()
            node.initializers = tuple(node.initializers[idx] for idx in uses)
        return RewriteResult(has_done_something=True)
