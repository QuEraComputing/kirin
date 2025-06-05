from ... import ir
from .stmts import For, Yield, IfElse
from ...rewrite.abc import RewriteRule, RewriteResult


class ScfToCfRule(RewriteRule):

    def rewrite_ifelse(
        self, node: ir.Region, block_idx: int, curr_block: ir.Block, stmt: IfElse
    ):
        from kirin.dialects import cf

        # create a new block for entering the if statement
        entry_block = ir.Block()
        for arg in curr_block.args:
            arg.replace_by(entry_block.args.append_from(arg.type, arg.name))

        # delete the args of the old block and replace with the result of the # if statement
        for arg in curr_block.args:
            curr_block.args.delete(arg)

        for arg in stmt.results:
            arg.replace_by(curr_block.args.append_from(arg.type, arg.name))

        (then_block := stmt.then_body.blocks[0]).detach()
        (else_block := stmt.else_body.blocks[0]).detach()

        entry_block.stmts.append(
            cf.ConditionalBranch(
                cond=stmt.cond,
                then_arguments=tuple(stmt.args),
                then_successor=then_block,
                else_arguments=tuple(stmt.args),
                else_successor=else_block,
            )
        )

        # insert the then/else blocks and add branch to the current block
        # if the last statement of the then block is a yield
        if isinstance(last_stmt := else_block.last_stmt, Yield):
            last_stmt.replace_by(
                cf.Branch(
                    arguments=tuple(last_stmt.args),
                    successor=curr_block,
                )
            )

        if isinstance(last_stmt := then_block.last_stmt, Yield):
            last_stmt.replace_by(
                cf.Branch(
                    arguments=tuple(last_stmt.args),
                    successor=curr_block,
                )
            )

        node.blocks.insert(block_idx, curr_block)
        node.blocks.insert(block_idx, else_block)
        node.blocks.insert(block_idx, then_block)

        curr_stmt = stmt
        next_stmt = stmt.prev_stmt
        curr_stmt.delete()

        return next_stmt, entry_block

    def rewrite_for(
        self, node: ir.Region, block_idx: int, curr_block: ir.Block, stmt: For
    ):
        from kirin.dialects import cf, py, func

        (body_block := stmt.body.blocks[0]).detach()

        entry_block = ir.Block()
        for arg in curr_block.args:
            arg.replace_by(entry_block.args.append_from(arg.type, arg.name))

        # Get iterator from iterable object
        entry_block.stmts.append(iterable_stmt := py.iterable.Iter(stmt.iterable))
        entry_block.stmts.append(const_none := func.ConstantNone())
        last_stmt = entry_block.last_stmt
        entry_block.stmts.append(
            next_stmt := py.iterable.Next(iterable_stmt.expect_one_result())
        )
        entry_block.stmts.append(
            loop_cmp := py.cmp.Is(next_stmt.expect_one_result(), const_none.result)
        )
        entry_block.stmts.append(
            cf.ConditionalBranch(
                cond=loop_cmp.result,
                then_arguments=tuple(stmt.initializers),
                then_successor=curr_block,
                else_arguments=(next_stmt.expect_one_result(),)
                + tuple(stmt.initializers),
                else_successor=body_block,
            )
        )

        for arg in curr_block.args:
            curr_block.args.delete(arg)

        for arg in stmt.results:
            arg.replace_by(curr_block.args.append_from(arg.type, arg.name))

        if isinstance(last_stmt := body_block.last_stmt, Yield):
            (
                next_stmt := py.iterable.Next(iterable_stmt.expect_one_result())
            ).insert_before(last_stmt)
            (
                loop_cmp := py.cmp.Is(next_stmt.expect_one_result(), const_none.result)
            ).insert_before(last_stmt)
            last_stmt.replace_by(
                cf.ConditionalBranch(
                    cond=loop_cmp.result,
                    else_arguments=(next_stmt.expect_one_result(),)
                    + tuple(last_stmt.args),
                    else_successor=body_block,
                    then_arguments=tuple(last_stmt.args),
                    then_successor=curr_block,
                )
            )

        # insert the body block and add branch to the current block
        node.blocks.insert(block_idx, curr_block)
        node.blocks.insert(block_idx, body_block)

        curr_stmt = stmt
        next_stmt = stmt.prev_stmt
        curr_stmt.delete()

        return next_stmt, entry_block

    def rewrite_ssacfg(self, node: ir.Region):

        has_done_something = False

        for block_idx in range(len(node.blocks)):

            block = node.blocks.pop(block_idx)

            stmt = block.last_stmt
            if stmt is None:
                continue

            curr_block = ir.Block()

            for arg in block.args:
                arg.replace_by(curr_block.args.append_from(arg.type, arg.name))

            while stmt is not None:
                if isinstance(stmt, For):
                    has_done_something = True
                    stmt, curr_block = self.rewrite_for(
                        node, block_idx, curr_block, stmt
                    )

                elif isinstance(stmt, IfElse):
                    has_done_something = True
                    stmt, curr_block = self.rewrite_ifelse(
                        node, block_idx, curr_block, stmt
                    )
                else:
                    curr_stmt = stmt
                    stmt = stmt.prev_stmt
                    curr_stmt.detach()

                    if curr_block.first_stmt is None:
                        curr_block.stmts.append(curr_stmt)
                    else:
                        curr_stmt.insert_before(curr_block.first_stmt)

            # if the last block is empty, remove it
            if curr_block.parent is None and curr_block.first_stmt is not None:
                node.blocks.insert(block_idx, curr_block)

        return RewriteResult(has_done_something=has_done_something)

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if (
            isinstance(node, (For, IfElse))
            or not node.has_trait(ir.HasCFG)
            and not node.has_trait(ir.SSACFG)
        ):
            # do not do rewrite in scf regions
            return RewriteResult()

        result = RewriteResult()
        for region in node.regions:
            result = result.join(self.rewrite_ssacfg(region))

        return result
