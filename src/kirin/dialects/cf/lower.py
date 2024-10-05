import ast

from kirin import ir
from kirin.dialects import cf
from kirin.dialects.py import stmts
from kirin.exceptions import DialectLoweringError
from kirin.lowering import Frame, FromPythonAST, LoweringState, Result


@cf.dialect.register
class CfLowering(FromPythonAST):

    def lower_Pass(self, ctx: LoweringState, node: ast.Pass) -> Result:
        next = ctx.current_frame.next_block
        if next is None:
            raise DialectLoweringError("code block is not exiting")
        ctx.append_stmt(cf.Branch(arguments=(), successor=next))
        return Result()

    def lower_Assert(self, ctx: LoweringState, node: ast.Assert) -> Result:
        cond = ctx.visit(node.test).expect_one()
        if node.msg:
            message = ctx.visit(node.msg).expect_one()
            ctx.append_stmt(cf.Assert(condition=cond, message=message))
        else:
            message_stmt = ctx.append_stmt(stmts.Constant(""))
            ctx.append_stmt(cf.Assert(condition=cond, message=message_stmt.result))
        return Result()

    def lower_If(self, ctx: LoweringState, node: ast.If) -> Result:
        cond = ctx.visit(node.test).expect_one()

        frame = ctx.current_frame
        before_block = frame.current_block
        if frame.next_block is None:
            raise DialectLoweringError("code block is not exiting")
        else:
            before_block_next = frame.next_block

        # NOTE: this if statement has code block after it
        # thus we need to create a new block for the next block
        # but we want to set the next block back to original after
        # the if statement, so whatever afterwards points to the
        # exit block
        if frame.stream.has_next():
            frame.next_block = ir.Block()  # don't push this yet

        frame_if = self.frame_if(ctx, frame, node)
        frame_else = self.frame_else(ctx, frame, node)
        self.frame_after_ifelse(ctx, frame, frame_if, frame_else, before_block_next)

        # insert branches to the last block of "if body" if no terminator
        if (
            frame_if.current_block.last_stmt is None
            or not frame_if.current_block.last_stmt.has_trait(ir.IsTerminator)
        ):
            if_args: list[ir.SSAValue] = []
            for arg in frame.next_block.args:
                if arg.name and (value := frame_if.get(arg.name)) is not None:
                    if_args.append(value)
                else:
                    raise DialectLoweringError(
                        f"unexpected argument without name: {arg}"
                    )
            frame_if.current_block.stmts.append(
                cf.Branch(arguments=tuple(if_args), successor=frame.next_block)
            )

        if frame_else:
            before_block.stmts.append(
                cf.ConditionalBranch(
                    cond=cond,
                    then_arguments=(),
                    then_successor=frame_if.entry_block,
                    else_arguments=(),
                    else_successor=frame_else.entry_block,
                )
            )

            if (
                frame_else.current_block.last_stmt is None
                or not frame_else.current_block.last_stmt.has_trait(ir.IsTerminator)
            ) and frame.next_block is not None:
                else_args: list[ir.SSAValue] = []
                for arg in frame.next_block.args:
                    if arg.name and (value := frame_else.get(arg.name)) is not None:
                        else_args.append(value)
                    else:
                        raise DialectLoweringError(
                            f"unexpected argument without name: {arg}"
                        )
                frame_else.current_block.stmts.append(
                    cf.Branch(arguments=tuple(else_args), successor=frame.next_block)
                )
        else:
            # no else body, when cond is false,
            # after body can only access parent frame
            # thus no input arguments for entry block
            # all other values are not cf dependent
            before_block.stmts.append(
                cf.ConditionalBranch(
                    cond=cond,
                    then_arguments=(),
                    then_successor=frame_if.entry_block,
                    else_arguments=(),
                    else_successor=frame.next_block,
                )
            )

        # NOTE: Python if statement has no return value
        frame.next_block = before_block_next
        return Result()

    def frame_if(self, state: LoweringState, frame: Frame, node: ast.If):
        frame_if = state.push_frame(
            Frame.from_stmts(
                node.body,
                state,
                region=frame.current_region,
                globals=frame.globals,
            )
        )
        frame_if.next_block = frame.next_block
        frame.current_block = frame_if.current_block
        state.exhaust(frame_if)
        state.pop_frame()
        return frame_if

    def frame_else(self, state: LoweringState, frame: Frame, node: ast.If):
        if not node.orelse:
            return

        frame_else = state.push_frame(
            Frame.from_stmts(
                node.orelse,
                state,
                region=frame.current_region,
                globals=frame.globals,
            )
        )
        frame_else.next_block = frame.next_block
        frame.current_block = frame_else.current_block
        state.exhaust(frame_else)
        state.pop_frame()
        return frame_else

    def frame_after_ifelse(
        self,
        state: LoweringState,
        frame: Frame,
        frame_if: Frame,
        frame_else: Frame | None,
        before_block_next: ir.Block,
    ):
        if not frame.stream.has_next():
            return

        defs: dict[str, set[ir.SSAValue]] = {}
        for name, value in frame_if.defs.items():
            phi = defs.setdefault(name, set())
            if isinstance(value, set):
                phi.update(value)
            else:
                phi.add(value)

        if frame_else:
            for name, value in frame_else.defs.items():
                phi = defs.setdefault(name, set())
                if isinstance(value, set):
                    phi.update(value)
                else:
                    phi.add(value)

        frame.defs.update(defs)
        frame_after = state.push_frame(
            Frame.from_stmts(
                frame.stream.split(),
                state,
                region=frame.current_region,
                block=frame.next_block,  # start from parent exit block
                globals=frame.globals,
            )
        )
        frame_after.next_block = before_block_next
        frame.current_block = frame_after.current_block
        state.exhaust(frame_after)
        state.pop_frame()
        return frame_after
