"""Lowering Python AST to cf dialect.
"""

import ast

from kirin import ir
from kirin.dialects import cf, py
from kirin.lowering import Frame, Result, FromPythonAST, LoweringState
from kirin.exceptions import DialectLoweringError

dialect = ir.Dialect("lowering.cf")


@dialect.register
class CfLowering(FromPythonAST):

    def lower_Pass(self, state: LoweringState, node: ast.Pass) -> Result:
        next = state.current_frame.next_block
        if next is None:
            raise DialectLoweringError("code block is not exiting")
        state.append_stmt(cf.Branch(arguments=(), successor=next))
        return Result()

    def lower_For(self, state: LoweringState, node: ast.For) -> Result:
        iterable = state.visit(node.iter).expect_one()
        iter_stmt = state.append_stmt(py.iterable.Iter(iterable))
        none_stmt = state.append_stmt(py.Constant(None))
        yields: list[str] = []

        def new_block_arg_if_inside_loop(frame: Frame, capture: ir.SSAValue):
            if not capture.name:
                raise DialectLoweringError("unexpected loop variable captured")
            yields.append(capture.name)
            return frame.entry_block.args.append_from(capture.type, capture.name)

        frame = state.current_frame
        next_block = ir.Block()

        body_frame = state.push_frame(
            Frame.from_stmts(
                node.body,
                state,
                region=frame.current_region,
                globals=state.current_frame.globals,
                capture_callback=new_block_arg_if_inside_loop,
            )
        )
        body_frame.next_block = next_block
        next_value = body_frame.entry_block.args.append_from(ir.types.Any, "next_value")
        py.unpack.unpackable(state, node.target, next_value)
        state.exhaust(body_frame)
        next_stmt = body_frame.append_stmt(py.iterable.Next(iter_stmt.iter))
        cond_stmt = body_frame.append_stmt(py.cmp.Is(next_stmt.value, none_stmt.result))
        yield_args = tuple(body_frame.get_scope(name) for name in yields)
        state.pop_frame()

        next_frame = state.push_frame(
            Frame.from_stmts(
                frame.stream.split(),
                state,
                region=frame.current_region,
                block=next_block,
                globals=frame.globals,
            )
        )
        next_frame.next_block = frame.next_block
        for name, arg in zip(yields, yield_args):
            input = next_frame.current_block.args.append_from(arg.type, name)
            next_frame.defs[name] = input
        state.exhaust()
        state.pop_frame()

        yield_args = tuple(body_frame.get_scope(name) for name in yields)
        body_frame.append_stmt(
            cf.ConditionalBranch(
                cond_stmt.result,
                yield_args,
                (next_stmt.value,) + yield_args,
                then_successor=next_frame.entry_block,
                else_successor=body_frame.entry_block,
            )
        )

        next_stmt = frame.append_stmt(py.iterable.Next(iter_stmt.iter))
        cond_stmt = frame.append_stmt(py.cmp.Is(next_stmt.value, none_stmt.result))
        yield_args = tuple(frame.get_scope(name) for name in yields)
        frame.append_stmt(
            cf.ConditionalBranch(
                cond_stmt.result,
                yield_args,
                (next_stmt.value,) + yield_args,
                then_successor=next_frame.entry_block,
                else_successor=body_frame.entry_block,
            )
        )
        frame.current_block = next_frame.current_block
        frame.next_block = next_frame.next_block
        frame.defs.update(next_frame.defs)
        return Result()

    def lower_If(self, state: LoweringState, node: ast.If) -> Result:
        cond = state.visit(node.test).expect_one()

        frame = state.current_frame
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

        frame_if = self.frame_if(state, frame, cond, node)
        frame_else = self.frame_else(state, frame, cond, node)
        self.frame_after_ifelse(state, frame, frame_if, frame_else, before_block_next)

        # insert branches to the last block of "if body" if no terminator
        if (
            frame_if.current_block.last_stmt is None
            or not frame_if.current_block.last_stmt.has_trait(ir.IsTerminator)
        ):
            frame_if.current_block.stmts.append(
                cf.Branch(
                    arguments=self.lookup_block_args(frame_if, frame.next_block),
                    successor=frame.next_block,
                )
            )

        if frame_else:
            before_block.stmts.append(
                cf.ConditionalBranch(
                    cond=cond,
                    then_arguments=self.lookup_block_args(frame, frame_if.entry_block),
                    then_successor=frame_if.entry_block,
                    else_arguments=self.lookup_block_args(
                        frame, frame_else.entry_block
                    ),
                    else_successor=frame_else.entry_block,
                )
            )

            if (
                frame_else.current_block.last_stmt is None
                or not frame_else.current_block.last_stmt.has_trait(ir.IsTerminator)
            ) and frame.next_block is not None:
                frame_else.current_block.stmts.append(
                    cf.Branch(
                        arguments=self.lookup_block_args(frame_else, frame.next_block),
                        successor=frame.next_block,
                    )
                )
        else:
            # no else body, when cond is false,
            # after body can only access parent frame
            # thus no input arguments for entry block
            # all other values are not cf dependent
            before_block.stmts.append(
                cf.ConditionalBranch(
                    cond=cond,
                    then_arguments=self.lookup_block_args(frame, frame_if.entry_block),
                    then_successor=frame_if.entry_block,
                    else_arguments=self.lookup_block_args(frame, frame.next_block),
                    else_successor=frame.next_block,
                )
            )

        # NOTE: Python if statement has no return value
        frame.next_block = before_block_next
        return Result()

    def frame_if(
        self, state: LoweringState, frame: Frame, cond: ir.SSAValue, node: ast.If
    ):
        frame_if = state.push_frame(
            Frame.from_stmts(
                node.body,
                state,
                region=frame.current_region,
                globals=frame.globals,
            )
        )
        frame_if.defs[cond.name] = set([cond])  # type: ignore
        frame_if.next_block = frame.next_block
        frame.current_block = frame_if.current_block
        state.exhaust(frame_if)
        state.pop_frame()
        return frame_if

    def frame_else(
        self, state: LoweringState, frame: Frame, cond: ir.SSAValue, node: ast.If
    ):
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
        frame_else.defs[cond.name] = set([cond])  # type: ignore
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

    @staticmethod
    def lookup_block_args(frame: Frame, block: ir.Block):
        args: list[ir.SSAValue] = []
        for arg in block.args:
            if arg.name and (value := frame.get(arg.name)) is not None:
                args.append(value)
            else:
                raise DialectLoweringError(f"unexpected argument without name: {arg}")
        return tuple(args)
