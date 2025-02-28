from typing import Callable
from dataclasses import dataclass

from kirin import ir
from kirin.dialects import cf, func
from kirin.rewrite.abc import RewriteRule, RewriteResult

# TODO: use func.Constant instead of kirin.dialects.py.stmts.Constant
from kirin.dialects.py.constant import Constant

# NOTE: this only inlines func dialect


@dataclass
class Inline(RewriteRule):
    heuristic: Callable[[ir.Statement], bool]
    """inline heuristic that determines whether a function should be inlined
    """

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if isinstance(node, func.Invoke):
            return self.rewrite_func_Invoke(node)
        elif isinstance(node, func.Call):
            return self.rewrite_func_Call(node)
        else:
            return RewriteResult()

    def rewrite_func_Call(self, node: func.Call) -> RewriteResult:
        if not isinstance(lambda_stmt := node.callee.owner, func.Lambda):
            return RewriteResult()

        # NOTE: a lambda statement is defined and used in the same scope
        self.inline_call_like(node, tuple(node.args), lambda_stmt.body)
        return RewriteResult(has_done_something=True)

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
            func_self = Constant(node.callee)
            func_self.result.name = node.callee.sym_name
            func_self.insert_before(node)
            self.inline_call_like(
                node, (func_self.result,) + tuple(arg for arg in node.args), region
            )
            has_done_something = True

        return RewriteResult(has_done_something=has_done_something)

    def inline_call_like(
        self,
        call_like: ir.Statement,
        args: tuple[ir.SSAValue, ...],
        region: ir.Region,
    ):
        """
        Inline a function call-like statement

        Args:
            call_like (ir.Statement): the call-like statement
            args (tuple[ir.SSAValue, ...]): the arguments of the call (first one is the callee)
            region (ir.Region): the region of the callee
        """
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
        if not call_like.parent_block:
            return

        if not call_like.parent_region:
            return

        # NOTE: we cannot change region because it may be used elsewhere
        inline_region: ir.Region = region.clone()
        parent_block: ir.Block = call_like.parent_block
        parent_region: ir.Region = call_like.parent_region

        # wrap what's after invoke into a block
        after_block = ir.Block()
        stmt = call_like.next_stmt
        while stmt is not None:
            stmt.detach()
            after_block.stmts.append(stmt)
            stmt = call_like.next_stmt

        for result in call_like.results:
            block_arg = after_block.args.append_from(result.type, result.name)
            result.replace_by(block_arg)

        parent_block_idx = parent_region._block_idx[parent_block]
        entry_block = inline_region.blocks.popfirst()
        idx, block = 0, entry_block
        while block is not None:
            idx += 1

            if block.last_stmt and isinstance(block.last_stmt, func.Return):
                block.last_stmt.replace_by(
                    cf.Branch(
                        arguments=(block.last_stmt.value,),
                        successor=after_block,
                    )
                )

            parent_region.blocks.insert(parent_block_idx + idx, block)
            block = inline_region.blocks.popfirst()

        parent_region.blocks.append(after_block)

        # NOTE: we expect always to have an entry block
        # but we still check for it cuz we are not gonna
        # error for empty regions here.
        if entry_block:
            cf.Branch(
                arguments=args,
                successor=entry_block,
            ).insert_before(call_like)
        call_like.delete()
        return
