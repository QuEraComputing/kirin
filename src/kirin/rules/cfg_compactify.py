from dataclasses import dataclass

from kirin import ir
from kirin.analysis.cfg import CFG
from kirin.dialects import cf
from kirin.rewrite import RewriteResult, RewriteRule


@dataclass
class CFGCompactify(RewriteRule):
    cfg: CFG

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if (trait := node.get_trait(ir.SSACFGRegion)) is None:
            return RewriteResult()

        has_done_something = False
        for region in node.regions:
            cfg = trait.get_graph(region)
            result = CFGCompactify(cfg).rewrite(region)
            has_done_something = has_done_something or result.has_done_something

        return RewriteResult(has_done_something=has_done_something)

    def rewrite_Region(self, node: ir.Region) -> RewriteResult:
        has_done_something = False
        for block in node.blocks:
            result = self.rewrite_Block(block)
            has_done_something = has_done_something or result.has_done_something

        # only traverse the first level of stmts
        for block in node.blocks:
            for stmt in block.stmts:
                result = self.rewrite_Statement(stmt)
                has_done_something = has_done_something or result.has_done_something

        return RewriteResult(has_done_something=has_done_something)

    def rewrite_Block(self, node: ir.Block) -> RewriteResult:
        if node not in self.cfg.successors:
            node.delete()
            return RewriteResult(has_done_something=True)

        if len(node.stmts) == 1:
            return self._merge_single_stmt_block(node)

        successors = self.cfg.successors[node]
        if len(successors) > 1:
            return RewriteResult()

        # block has only one successor
        # if the terminator is cf.Branch
        # append its successor to this block
        # and remove the terminator, the new
        # terminator will be the successor's terminator
        if isinstance(terminator := node.last_stmt, cf.Branch):
            successor = terminator.successor
            # not single edge in CFG
            if len(self.cfg.predecessors[successor]) == 1:
                return self._merge_one_way_edges(node, terminator, successor)
            return RewriteResult()
        else:
            return RewriteResult()

    def _merge_single_stmt_block(self, node: ir.Block):
        if isinstance(stmt := node.last_stmt, cf.Branch):
            return self._merge_single_Branch_block(node, stmt)
        return RewriteResult()

    def _merge_one_way_edges(
        self, node: ir.Block, terminator: cf.Branch, successor: ir.Block
    ):
        stmt = successor.first_stmt
        while stmt is not None:
            curr = stmt
            stmt = stmt.next_stmt
            curr.detach()
            node.stmts.append(curr)

        for arg, input in zip(successor.args, terminator.arguments):
            arg.replace_by(input)
        terminator.delete()
        successor.delete()
        return RewriteResult(has_done_something=True)

    def _merge_single_Branch_block(
        self, block: ir.Block, branch: cf.Branch
    ) -> RewriteResult:
        # NOTE: has no predecessors, but not deadblock
        # must be entry block
        if block not in self.cfg.predecessors:
            return self._merge_single_Branch_entry_block(block, branch)

        has_other_predecessors = False
        has_done_something = False
        for predecessor in self.cfg.predecessors[block]:
            # NOTE: if the predecessor terminator is cf.Branch,
            # we will use _merge_one_way_edges to merge the blocks
            # because it will be one way no matter how many stmts
            # are there inside the block: [A] -> [B] -> [C]
            if not isinstance(predecessor.last_stmt, cf.ConditionalBranch):
                has_other_predecessors = True
                continue

            if predecessor.last_stmt.then_successor is block:
                ssamap = self._record_block_inputs(
                    block, predecessor.last_stmt.then_arguments
                )
                stmt = cf.ConditionalBranch(
                    cond=predecessor.last_stmt.cond,
                    then_arguments=tuple(
                        ssamap.get(arg, arg) for arg in branch.arguments
                    ),
                    then_successor=branch.successor,
                    else_arguments=predecessor.last_stmt.else_arguments,
                    else_successor=predecessor.last_stmt.else_successor,
                )
            else:  # predecessor.last_stmt.else_successor is block:
                ssamap = self._record_block_inputs(
                    block, predecessor.last_stmt.else_arguments
                )
                stmt = cf.ConditionalBranch(
                    cond=predecessor.last_stmt.cond,
                    then_arguments=predecessor.last_stmt.then_arguments,
                    then_successor=predecessor.last_stmt.then_successor,
                    else_arguments=tuple(
                        ssamap.get(arg, arg) for arg in branch.arguments
                    ),
                    else_successor=branch.successor,
                )

            predecessor.last_stmt.replace_by(stmt)
            has_done_something = True

        if not has_other_predecessors:
            block.delete()
            return RewriteResult(has_done_something=True)
        return RewriteResult(has_done_something=has_done_something)

    def _merge_single_Branch_entry_block(self, block: ir.Block, stmt: cf.Branch):
        for arg in block.args:
            new_arg = stmt.successor.args.append_from(arg.type, arg.name)
            arg.replace_by(new_arg)
        block.delete()
        return RewriteResult(has_done_something=True)

    @staticmethod
    def _record_block_inputs(block: ir.Block, arguments: tuple[ir.SSAValue, ...]):
        ssamap: dict[ir.SSAValue, ir.SSAValue] = {}
        for arg, input in zip(arguments, block.args):
            ssamap[arg] = input
        return ssamap
