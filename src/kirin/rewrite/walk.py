from typing import Callable
from dataclasses import field, dataclass

from kirin.ir import Block, Region, Statement
from kirin.worklist import WorkList
from kirin.rewrite.abc import RewriteRule
from kirin.ir.nodes.base import IRNode
from kirin.rewrite.result import RewriteResult


@dataclass
class Walk(RewriteRule):
    """Walk through the IR nodes and apply a rewrite rule.

    The walk will apply the rewrite rule to each node in the IR tree in a depth-first order.

    ### Parameters
    - `map`: The rewrite rule to apply.
    - `reverse`: Whether to traverse the IR tree in reverse order. Default is `False`.
    - `region_first`: Whether to traverse the regions before the blocks. Default is `False`.
    """

    rule: RewriteRule
    worklist: WorkList[IRNode] = field(default_factory=WorkList)
    skip: Callable[[IRNode], bool] = field(default=lambda _: False)

    # options
    reverse: bool = field(default=False)
    region_first: bool = field(default=False)

    def rewrite(self, node: IRNode) -> RewriteResult:
        # NOTE: because the rewrite pass may mutate the node
        # thus we need to save the list of nodes to be processed
        # first before we start processing them
        self.populate_worklist(node)
        has_done_something = False
        subnode = self.worklist.pop()
        while subnode is not None:
            result = self.rule.rewrite(subnode)
            if result.terminated:
                return result

            if result.has_done_something:
                has_done_something = True
            subnode = self.worklist.pop()
        return RewriteResult(has_done_something=has_done_something)

    def populate_worklist(self, node: IRNode) -> None:
        if self.skip(node):
            return

        if isinstance(node, Statement):
            self.populate_worklist_Statement(node)
        elif isinstance(node, Region):
            self.populate_worklist_Region(node)
        elif isinstance(node, Block):
            self.populate_worklist_Block(node)
        else:
            raise NotImplementedError(f"populate_worklist_{node.__class__.__name__}")

    def populate_worklist_Statement(self, node: Statement) -> None:
        if self.region_first:
            self.worklist.append(node)

        if node.regions:
            for region in reversed(node.regions) if not self.reverse else node.regions:
                self.populate_worklist(region)

        if not self.region_first:
            self.worklist.append(node)

    def populate_worklist_Region(self, node: Region) -> None:
        self.worklist.append(node)
        if node.blocks:
            for block in reversed(node.blocks) if not self.reverse else node.blocks:
                self.populate_worklist(block)

    def populate_worklist_Block(self, node: Block) -> None:
        self.worklist.append(node)
        stmt = node.last_stmt
        while stmt is not None:
            self.populate_worklist(stmt)
            stmt = stmt.prev_stmt
