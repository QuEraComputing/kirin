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

        # for For loops, keep loop-carried variables whose block arguments
        # are used inside the body AND are actually mutated across iterations,
        # even if the result is unused after the loop. A variable that is just
        # passed through (yielded unchanged) can safely be replaced by its
        # initializer.
        if isinstance(node, For):
            block = node.body.blocks[0]
            yield_stmt = block.last_stmt
            for idx in range(len(node.initializers)):
                if idx not in uses and block.args[idx + 1].uses:
                    # Check if the variable is mutated: the yielded value
                    # differs from the block argument (not just passed through)
                    if (
                        isinstance(yield_stmt, Yield)
                        and yield_stmt.args[idx] is not block.args[idx + 1]
                    ):
                        uses.add(idx)
            results = [r for idx, r in enumerate(node._results) if idx in uses]
            if len(results) == len(node._results):
                return RewriteResult()

        node._results = results
        for region in node.regions:
            for block in region.blocks:
                if not isinstance(block.last_stmt, Yield):
                    continue
                # remove unused results from the yield statement
                block.last_stmt.args = [block.last_stmt.args[idx] for idx in uses]

        if isinstance(node, For):
            # replace the block arguments at the unused indices with the initializers
            # this works because the initializers are coming from the parent region of the For
            not_used = set(range(len(node.initializers))) - uses
            block = node.body.blocks[0]
            args_to_delete: list[ir.BlockArgument] = []
            for idx in not_used:
                block_arg = block.args[idx + 1]
                block_arg.replace_by(node.initializers[idx])
                args_to_delete.append(block_arg)

            for arg in args_to_delete:
                arg.delete()

            # remove the unused initializers from the initializers inputs
            node.initializers = tuple(node.initializers[idx] for idx in uses)

        return RewriteResult(has_done_something=True)
