from dataclasses import dataclass
from typing import Callable

from kirin import ir
from kirin.dialects import cf, func

# TODO: use func.Constant instead of kirin.dialects.py.stmts.Constant
from kirin.dialects.py import stmts
from kirin.rewrite import RewriteResult, RewriteRule

# NOTE: this only inlines func dialect


@dataclass
class Inline(RewriteRule):
    heuristic: Callable[[ir.IRNode], bool]
    """inline heuristic that determines whether a function should be inlined
    """

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if isinstance(node, func.Invoke):
            return self.rewrite_func_Invoke(node)
        else:
            return RewriteResult()

    def rewrite_func_Invoke(self, node: func.Invoke) -> RewriteResult:
        has_done_something = False
        callee = node.callee

        if (
            isinstance(callee, ir.Method)
            and self.heuristic(callee.code)
            and (call_trait := callee.code.get_trait(ir.CallableStmtInterface))
            is not None
        ):
            region = call_trait.get_callable_region(callee.code)
            self.inline_callee(node, region)
            has_done_something = True

        return RewriteResult(has_done_something=has_done_something)

    def inline_callee(self, call: func.Invoke, region: ir.Region):
        # <stmt>
        # <stmt>
        # <br (a, b, c)>

        # <block (a, b,c)>:
        # <block>:
        # <block>:
        # <br>

        # ^<block>:
        # <stmt>
        # <stmt>

        # 1. we insert the entry block of the callee function
        # 2. we insert the rest of the blocks into the parent region
        # 3.1 if the return is in the entry block, means no control flow,
        #     replace the call results with the return values
        # 3.2 if the return is some of the blocks, means control flow,
        #     split the current block into two, and replace the return with
        #     the branch instruction
        # 4. remove the call
        if not call.parent_block:
            return

        if not call.parent_region:
            return

        parent_block: ir.Block = call.parent_block
        parent_region: ir.Region = call.parent_region

        after_block = ir.Block()
        stmt = call.next_stmt
        while stmt is not None:
            stmt.detach()
            after_block.stmts.append(stmt)
            stmt = call.next_stmt

        for result in call.results:
            block_arg = after_block.args.append_from(result.type, result.name)
            result.replace_by(block_arg)

        parent_block_idx = parent_region._block_idx[parent_block]
        # NOTE: we cannot change region because it may be used elsewhere
        entry_block = region.blocks[0].clone()
        parent_region.blocks.insert(parent_block_idx + 1, entry_block)
        if entry_block.last_stmt and isinstance(entry_block.last_stmt, func.Return):
            entry_block.last_stmt.replace_by(
                cf.Branch(
                    arguments=tuple(arg for arg in entry_block.last_stmt.args),
                    successor=after_block,
                )
            )
        for idx, block in enumerate(region.blocks[1:]):
            parent_region.blocks.insert(parent_block_idx + idx + 2, block.clone())

        parent_region.blocks.append(after_block)

        func_self = stmts.Constant(call.callee)
        func_self.result.name = call.callee.sym_name
        func_self.insert_before(call)
        cf.Branch(
            arguments=(func_self.result,) + tuple(arg for arg in call.args),
            successor=entry_block,
        ).insert_before(call)
        call.delete()
        return
