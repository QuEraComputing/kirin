"""Lowering Python AST to cf dialect."""

import ast

from kirin import ir, types
from kirin.dialects import cf, py
from kirin.lowering import Frame, Result, FromPythonAST, LoweringState
from kirin.exceptions import DialectLoweringError

dialect = ir.Dialect("lowering.cf")


@dialect.register
class CfLowering(FromPythonAST):

    def lower_Pass(self, state: LoweringState, node: ast.Pass) -> Result:
        state.append_stmt(
            cf.Branch(arguments=(), successor=state.current_frame.next_block)
        )
        return Result()

    def lower_For(self, state: LoweringState, node: ast.For) -> Result:
        yields: list[str] = []

        def new_block_arg_if_inside_loop(frame: Frame, capture: ir.SSAValue):
            if not capture.name:
                raise DialectLoweringError("unexpected loop variable captured")
            yields.append(capture.name)
            return frame.entr_block.args.append_from(capture.type, capture.name)

        frame = state.current_frame
        iterable = state.visit(node.iter).expect_one()
        iter_stmt = frame.append_stmt(py.iterable.Iter(iterable))
        none_stmt = frame.append_stmt(py.Constant(None))

        body_frame = state.push_frame(
            Frame.from_stmts(
                node.body,
                state,
                region=state.current_frame.curr_region,
                globals=state.current_frame.globals,
                capture_callback=new_block_arg_if_inside_loop,
            )
        )
        next_value = body_frame.entr_block.args.append_from(types.Any, "next_value")
        py.unpack.unpacking(state, node.target, next_value)
        state.exhaust(body_frame)
        self.branch_next_if_not_terminated(body_frame)
        yield_args = tuple(body_frame.get_scope(name) for name in yields)
        next_stmt = py.iterable.Next(iter_stmt.iter)
        cond_stmt = py.cmp.Is(next_stmt.value, none_stmt.result)
        body_frame.next_block.stmts.append(next_stmt)
        body_frame.next_block.stmts.append(cond_stmt)
        body_frame.next_block.stmts.append(
            cf.ConditionalBranch(
                cond_stmt.result,
                yield_args,
                (next_stmt.value,) + yield_args,
                then_successor=frame.next_block,
                else_successor=body_frame.entr_block,
            )
        )
        state.pop_frame()

        # insert the branch to the entrance of the loop (the code block before loop)
        next_stmt = frame.append_stmt(py.iterable.Next(iter_stmt.iter))
        cond_stmt = frame.append_stmt(py.cmp.Is(next_stmt.value, none_stmt.result))
        yield_args = tuple(frame.get_scope(name) for name in yields)
        frame.append_stmt(
            cf.ConditionalBranch(
                cond_stmt.result,
                yield_args,
                (next_stmt.value,) + yield_args,
                then_successor=frame.next_block,  # empty iterator
                else_successor=body_frame.entr_block,
            )
        )

        frame.jump_next()
        for name, arg in zip(yields, yield_args):
            input = frame.curr_block.args.append_from(arg.type, name)
            frame.defs[name] = input
        return Result()

    def lower_If(self, state: LoweringState, node: ast.If) -> Result:
        cond = state.visit(node.test).expect_one()
        frame = state.current_frame
        before_block = frame.curr_block
        if_frame = state.push_frame(
            Frame.from_stmts(
                node.body,
                state,
                region=frame.curr_region,
                globals=frame.globals,
            )
        )
        true_cond = if_frame.entr_block.args.append_from(types.Bool, cond.name)
        if cond.name:
            if_frame.defs[cond.name] = true_cond
        state.exhaust()
        self.branch_next_if_not_terminated(if_frame)
        state.pop_frame()

        else_frame = state.push_frame(
            Frame.from_stmts(
                node.orelse,
                state,
                region=frame.curr_region,
                globals=frame.globals,
            )
        )
        true_cond = else_frame.entr_block.args.append_from(types.Bool, cond.name)
        if cond.name:
            else_frame.defs[cond.name] = true_cond
        state.exhaust()
        self.branch_next_if_not_terminated(else_frame)
        state.pop_frame()

        after_frame = state.push_frame(
            Frame.from_stmts(
                frame.stream.split(),
                state,
                region=frame.curr_region,
                globals=frame.globals,
            )
        )

        after_frame.defs.update(frame.defs)
        phi: set[str] = set()
        for name in if_frame.defs.keys():
            if frame.get(name):
                phi.add(name)
            elif name in else_frame.defs:
                phi.add(name)

        for name in else_frame.defs.keys():
            if frame.get(name):  # not defined in if_frame
                phi.add(name)

        for name in phi:
            after_frame.defs[name] = after_frame.entr_block.args.append_from(
                types.Any, name
            )

        state.exhaust()
        self.branch_next_if_not_terminated(after_frame)
        after_frame.next_block.stmts.append(
            cf.Branch(arguments=(), successor=frame.next_block)
        )
        state.pop_frame()

        if_args = []
        for name in phi:
            if value := if_frame.get(name):
                if_args.append(value)
            else:
                raise DialectLoweringError(f"undefined variable {name} in if branch")

        else_args = []
        for name in phi:
            if value := else_frame.get(name):
                else_args.append(value)
            else:
                raise DialectLoweringError(f"undefined variable {name} in else branch")

        if_frame.next_block.stmts.append(
            cf.Branch(
                arguments=tuple(if_args),
                successor=after_frame.entr_block,
            )
        )
        else_frame.next_block.stmts.append(
            cf.Branch(
                arguments=tuple(else_args),
                successor=after_frame.entr_block,
            )
        )
        before_block.stmts.append(
            cf.ConditionalBranch(
                cond=cond,
                then_arguments=(cond,),
                then_successor=if_frame.entr_block,
                else_arguments=(cond,),
                else_successor=else_frame.entr_block,
            )
        )
        frame.jump_next()
        return Result()

    def branch_next_if_not_terminated(self, frame: Frame):
        """Branch to the next block if the current block is not terminated.

        This must be used after exhausting the current frame and before popping the frame.
        """
        if not frame.curr_block.last_stmt or not frame.curr_block.last_stmt.has_trait(
            ir.IsTerminator
        ):
            frame.curr_block.stmts.append(
                cf.Branch(arguments=(), successor=frame.next_block)
            )

    def current_block_terminated(self, frame: Frame):
        return frame.curr_block.last_stmt and frame.curr_block.last_stmt.has_trait(
            ir.IsTerminator
        )
