from kirin import ir
from kirin.analysis import const
from kirin.dialects import func
from kirin.rewrite.abc import RewriteRule
from kirin.rewrite.result import RewriteResult
from kirin.dialects.py.constant import Constant

from .stmts import For, Yield, IfElse


class PickIfElse(RewriteRule):

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, IfElse):
            return RewriteResult()

        if not isinstance(hint := node.cond.hints.get("const"), const.Value):
            return RewriteResult()

        if hint.data:
            return self.insert_body(node, node.then_body)
        else:
            return self.insert_body(node, node.else_body)

    def insert_body(self, node: IfElse, body: ir.Region):
        body_block = body.blocks[0]
        body_block.args[0].replace_by(node.cond)
        block_stmt = body_block.first_stmt
        while block_stmt and not block_stmt.has_trait(ir.IsTerminator):
            block_stmt.detach()
            block_stmt.insert_before(node)
            block_stmt = body_block.first_stmt

        terminator = body_block.last_stmt
        if isinstance(terminator, Yield):
            for result, output in zip(node.results, terminator.values):
                result.replace_by(output)
            node.delete()
            return RewriteResult(has_done_something=True)
        elif isinstance(terminator, func.Return):
            block = node.parent
            assert block is not None
            stmt = block.last_stmt
            while stmt is not None and stmt is not node:  # remove the rest of the block
                delete_stmt = stmt
                stmt = stmt.prev_stmt
                delete_stmt.delete()

            terminator.detach()
            node.replace_by(terminator)
            return RewriteResult(has_done_something=True)
        else:
            return RewriteResult()


class ForLoop(RewriteRule):

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, For):
            return RewriteResult()

        # TODO: support for PartialTuple and IList with known length
        if not isinstance(hint := node.iterable.hints.get("const"), const.Value):
            return RewriteResult()

        loop_vars = node.initializers
        for item in hint.data:
            body = node.body.clone()
            block = body.blocks[0]
            item_stmt = Constant(item)
            item_stmt.insert_before(node)
            block.args[0].replace_by(item_stmt.result)
            for var, input in zip(block.args[1:], loop_vars):
                var.replace_by(input)

            block_stmt = block.first_stmt
            while block_stmt and not block_stmt.has_trait(ir.IsTerminator):
                block_stmt.detach()
                block_stmt.insert_before(node)
                block_stmt = block.first_stmt

            terminator = block.last_stmt
            # we assume Yield has the same # of values as initializers
            # TODO: check this in validation
            if isinstance(terminator, Yield):
                loop_vars = terminator.values
                terminator.delete()

        for result, output in zip(node.results, loop_vars):
            result.replace_by(output)
        node.delete()
        return RewriteResult(has_done_something=True)
