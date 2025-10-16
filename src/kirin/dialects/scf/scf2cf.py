from dataclasses import field, dataclass

from kirin import ir
from kirin.rewrite.abc import RewriteRule, RewriteResult

from ..cf import Branch, ConditionalBranch
from ..func import ConstantNone
from .stmts import For, Yield, IfElse
from ..py.cmp import Is
from ..py.iterable import Iter, Next


class ScfRule(RewriteRule):

    def get_entr_and_exit_blks(self, node: For | IfElse):
        """Get the enter and exit blocks for the given SCF node.

        The exit block is a new block that will be created to hold the
        statements that follow the SCF node in the current block and the
        enter block is a new block that will be created to hold the
        any logic required to enter the SCF node.

        """
        # split the current block into two parts
        exit_block = ir.Block()
        stmt = node.next_stmt
        while stmt is not None:
            next_stmt = stmt.next_stmt
            stmt.detach()
            exit_block.stmts.append(stmt)
            stmt = next_stmt

        for result in node.results:
            result.replace_by(exit_block.args.append_from(result.type, result.name))

        curr_block = node.parent_block
        assert curr_block.IS_BLOCK, "Node must be inside a block"

        curr_block.stmts.append(
            Branch(arguments=(), successor=(entr_block := ir.Block()))
        )

        return exit_block, entr_block

    def get_curr_blk_info(self, node: For | IfElse) -> tuple[ir.Region, int]:
        """Get the current region and the block index of the node in the region."""
        curr_block = node.parent_block
        region = node.parent_region

        assert region.IS_REGION, "Node must be inside a region"
        assert curr_block.IS_BLOCK, "Node must be inside a block"

        block_idx = region._block_idx[curr_block]
        return region, block_idx


class ForRule(ScfRule):
    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if (
            not isinstance(node, For)
            # must be inside a callable statement
            or not isinstance(parent_stmt := node.parent_stmt, ir.Statement)
            or not parent_stmt.has_trait(ir.CallableStmtInterface)
        ):
            return RewriteResult()

        region, block_idx = self.get_curr_blk_info(node)
        exit_block, entr_block = self.get_entr_and_exit_blks(node)

        (body_block := node.body.blocks[0]).detach()

        # Get iterator from iterable object
        entr_block.stmts.append(iterable_stmt := Iter(node.iterable))
        entr_block.stmts.append(next_stmt := Next(iterable_stmt.expect_one_result()))
        entr_block.stmts.append(const_none := ConstantNone())
        entr_block.stmts.append(
            loop_cmp := Is(next_stmt.expect_one_result(), const_none.result)
        )
        entr_block.stmts.append(
            ConditionalBranch(
                cond=loop_cmp.result,
                then_arguments=tuple(node.initializers),
                then_successor=exit_block,
                else_arguments=(next_stmt.expect_one_result(),)
                + tuple(node.initializers),
                else_successor=body_block,
            )
        )

        if isinstance(last_stmt := body_block.last_stmt, Yield):
            (next_stmt := Next(iterable_stmt.expect_one_result())).insert_before(
                last_stmt
            )
            (const_none := ConstantNone()).insert_before(last_stmt)
            (
                loop_cmp := Is(next_stmt.expect_one_result(), const_none.result)
            ).insert_before(last_stmt)
            last_stmt.replace_by(
                ConditionalBranch(
                    cond=loop_cmp.result,
                    else_arguments=(next_stmt.expect_one_result(),)
                    + tuple(last_stmt.args),
                    else_successor=body_block,
                    then_arguments=tuple(last_stmt.args),
                    then_successor=exit_block,
                )
            )

        # insert the body block and add branch to the current block
        region.blocks.insert(block_idx + 1, exit_block)
        region.blocks.insert(block_idx + 1, body_block)
        region.blocks.insert(block_idx + 1, entr_block)

        node.delete()

        return RewriteResult(has_done_something=True)


class IfElseRule(ScfRule):
    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if (
            not isinstance(node, IfElse)
            or not isinstance(parent_stmt := node.parent_stmt, ir.Statement)
            or not parent_stmt.has_trait(ir.CallableStmtInterface)
        ):
            return RewriteResult()

        region, block_idx = self.get_curr_blk_info(node)
        exit_block, entr_block = self.get_entr_and_exit_blks(node)

        (then_block := node.then_body.blocks[0]).detach()
        (else_block := node.else_body.blocks[0]).detach()
        entr_block.stmts.append(
            ConditionalBranch(
                node.cond,
                then_arguments=tuple(node.args),
                then_successor=then_block,
                else_arguments=tuple(node.args),
                else_successor=else_block,
            )
        )

        if isinstance(last_stmt := then_block.last_stmt, Yield):
            last_stmt.replace_by(
                Branch(
                    arguments=tuple(last_stmt.args),
                    successor=exit_block,
                )
            )

        if isinstance(last_stmt := else_block.last_stmt, Yield):
            last_stmt.replace_by(
                Branch(
                    arguments=tuple(last_stmt.args),
                    successor=exit_block,
                )
            )

        # insert the new blocks
        region.blocks.insert(block_idx + 1, exit_block)
        region.blocks.insert(block_idx + 1, else_block)
        region.blocks.insert(block_idx + 1, then_block)
        region.blocks.insert(block_idx + 1, entr_block)

        node.delete()
        return RewriteResult(has_done_something=True)


@dataclass
class ScfToCfRule(RewriteRule):

    for_rule: ForRule = field(default_factory=ForRule, init=False)
    if_else_rule: IfElseRule = field(default_factory=IfElseRule, init=False)

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if isinstance(node, For):
            return self.for_rule.rewrite_Statement(node)
        elif isinstance(node, IfElse):
            return self.if_else_rule.rewrite_Statement(node)
        else:
            return RewriteResult()
