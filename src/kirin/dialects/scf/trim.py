from typing import Dict, List

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

    def get_equal_args(self, node: For) -> Dict[int, List[int]]:
        """Other iter_args indices that are equal to each index."""
        equal_args: Dict[int, List[int]] = {}
        original_iter_args = node.body.blocks[0].args[1:]
        original_yields = [
            block.last_stmt.args
            for region in node.regions
            for block in region.blocks
            if isinstance(block.last_stmt, Yield)
        ]

        for idx_1 in range(len(node.results)):
            equal_args[idx_1] = []
            for idx_2 in range(len(node.results)):
                are_equal = True
                if original_iter_args[idx_1].name != original_iter_args[idx_2].name:
                    are_equal = False
                if node.initializers[idx_1] != node.initializers[idx_2]:
                    are_equal = False
                for values in original_yields:
                    if values[idx_1] != values[idx_2]:
                        are_equal = False
                if are_equal and idx_1 != idx_2:
                    equal_args[idx_1].append(idx_2)
        return equal_args

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, (For, IfElse)):
            return RewriteResult()

        any_unused, uses, results = self.scan_unused(node)
        if not any_unused:
            return RewriteResult()

        all_idx = list(range(len(node.results)))
        idx_drop = [idx for idx in all_idx if idx not in uses]

        equal_args: Dict[int, List[int]] = {}
        if isinstance(node, For):
            equal_args = self.get_equal_args(node)
            equal_args = {
                idx: [other_idx for other_idx in others if other_idx not in idx_drop]
                for idx, others in equal_args.items()
            }

        node._results = results

        for region in node.regions:
            for block in region.blocks:
                if not isinstance(block.last_stmt, Yield):
                    continue

                block.last_stmt.args = [block.last_stmt.args[idx] for idx in uses]

        if isinstance(node, For):
            idx_arg_drop = set()
            iter_args = node.body.blocks[0].args[1:]
            for idx in idx_drop:
                if len(iter_args[idx].uses) == 0:
                    idx_arg_drop.add(idx)
                elif len(equal_args[idx]) == 0:
                    idx_arg_drop.add(idx)
                else:
                    for equal_idx in equal_args[idx]:
                        if len(iter_args[equal_idx].uses) == 0:
                            idx_arg_drop.add(equal_idx)
                            break
            idx_arg_keep = sorted(set(all_idx) - idx_arg_drop)

            for idx in sorted(idx_arg_drop):
                block_arg = iter_args[idx]
                block_arg.replace_by(node.initializers[idx])
                block_arg.delete()
            node.initializers = tuple(node.initializers[idx] for idx in idx_arg_keep)
        return RewriteResult(has_done_something=True)
