from dataclasses import dataclass

from kirin import ir
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.analysis.cfg import CFG


@dataclass
class SortBlocks(RewriteRule):
    """Reorder blocks in a region to reverse post-order of the CFG.

    RPO guarantees that in well-formed SSA, a block's dominator is visited
    before the block itself, so statement results appear before their uses
    in block-list order. This is required for correct ``Region.clone()``
    and benefits any pass that iterates blocks sequentially.
    """

    cfg: CFG

    def rewrite_Region(self, node: ir.Region) -> RewriteResult:
        # NOTE: relies on self.cfg being up-to-date. When used inside
        # CompactifyRegion, prior rules (DeadBlock, CFGEdge, etc.) mutate
        # the shared CFG's successors/predecessors dicts in place.
        successors = self.cfg.successors

        visited: set[ir.Block] = set()
        post_order: list[ir.Block] = []

        if self.cfg.entry is not None:
            stack: list[tuple[ir.Block, bool]] = [(self.cfg.entry, False)]
            while stack:
                block, returning = stack.pop()
                if returning:
                    post_order.append(block)
                    continue
                if block in visited:
                    continue
                visited.add(block)
                stack.append((block, True))
                for succ in successors.get(block, ()):
                    if succ not in visited:
                        stack.append((succ, False))

        post_order.reverse()

        # Append unreachable blocks in their original order.
        block_set = set(post_order)
        for block in node.blocks:
            if block not in block_set:
                post_order.append(block)

        if list(node.blocks) == post_order:
            return RewriteResult()

        # Reorder in place — blocks are already attached to the region,
        # so we update the internal list and index directly.
        node._blocks[:] = post_order
        node._block_idx = {block: i for i, block in enumerate(post_order)}
        return RewriteResult(has_done_something=True)
