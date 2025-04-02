"""Lowering Python AST to cf dialect."""

import ast

from kirin import ir, types, lowering
from kirin.dialects import cf, py

dialect = ir.Dialect("lowering.cf")


@dialect.register
class CfLowering(lowering.FromPythonAST):

    def lower_Pass(self, state: lowering.State, node: ast.Pass):
        state.current_frame.push(
            cf.Branch(arguments=(), successor=state.current_frame.next_block)
        )

    def lower_For(self, state: lowering.State, node: ast.For) -> lowering.Result:
        yields: list[str] = []

        def new_block_arg_if_inside_loop(frame: lowering.Frame, capture: ir.SSAValue):
            if not capture.name:
                raise lowering.BuildError("unexpected loop variable captured")
            yields.append(capture.name)
            return frame.entr_block.args.append_from(capture.type, capture.name)

        frame = state.current_frame
        iterable = state.lower(node.iter).expect_one()
        iter_stmt = frame.push(py.iterable.Iter(iterable))
        none_stmt = frame.push(py.Constant(None))

        with state.frame(
            node.body,
            region=state.current_frame.curr_region,
            capture_callback=new_block_arg_if_inside_loop,
        ) as body_frame:
            next_value = body_frame.entr_block.args.append_from(types.Any, "next_value")
            py.unpack.unpacking(state, node.target, next_value)
            body_frame.exhaust()
            self.branch_next_if_not_terminated(body_frame)
            yield_args = tuple(body_frame[name] for name in yields)
            next_stmt = py.iterable.Next(iter_stmt.iter)
            cond_stmt = py.cmp.Is(next_stmt.value, none_stmt.result)
            body_frame.next_block.stmts.append(next_stmt)
            body_frame.next_block.stmts.append(cond_stmt)
            body_frame.next_block.stmts.append(
                cf.ConditionalBranch(
                    cond_stmt.result,
                    then_arguments=yield_args,
                    else_arguments=(next_stmt.value,) + yield_args,
                    then_successor=frame.next_block,
                    else_successor=body_frame.entr_block,
                )
            )

        # insert the branch to the entrance of the loop (the code block before loop)
        next_stmt = frame.push(py.iterable.Next(iter_stmt.iter))
        cond_stmt = frame.push(py.cmp.Is(next_stmt.value, none_stmt.result))
        yield_args = tuple(frame[name] for name in yields)
        frame.push(
            cf.ConditionalBranch(
                cond_stmt.result,
                yield_args,
                (next_stmt.value,) + yield_args,
                then_successor=frame.next_block,  # empty iterator
                else_successor=body_frame.entr_block,
            )
        )

        frame.jump_next_block()
        for name, arg in zip(yields, yield_args):
            input = frame.curr_block.args.append_from(arg.type, name)
            frame.defs[name] = input

    def lower_If(self, state: lowering.State, node: ast.If) -> lowering.Result:
        cond = state.lower(node.test).expect_one()
        frame = state.current_frame
        before_block = frame.curr_block

        with state.frame(node.body, region=frame.curr_region) as if_frame:
            true_cond = if_frame.entr_block.args.append_from(types.Bool, cond.name)
            if cond.name:
                if_frame.defs[cond.name] = true_cond

            if_frame.exhaust()
            self.branch_next_if_not_terminated(if_frame)

        with state.frame(node.orelse, region=frame.curr_region) as else_frame:
            true_cond = else_frame.entr_block.args.append_from(types.Bool, cond.name)
            if cond.name:
                else_frame.defs[cond.name] = true_cond
            else_frame.exhaust()
            self.branch_next_if_not_terminated(else_frame)

        with state.frame(frame.stream.split(), region=frame.curr_region) as after_frame:
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

            after_frame.exhaust()
            self.branch_next_if_not_terminated(after_frame)
            after_frame.next_block.stmts.append(
                cf.Branch(arguments=(), successor=frame.next_block)
            )

        if_args = []
        for name in phi:
            if value := if_frame.get(name):
                if_args.append(value)
            else:
                raise lowering.BuildError(f"undefined variable {name} in if branch")

        else_args = []
        for name in phi:
            if value := else_frame.get(name):
                else_args.append(value)
            else:
                raise lowering.BuildError(f"undefined variable {name} in else branch")

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
        frame.defs.update(after_frame.defs)
        frame.jump_next_block()

    def branch_next_if_not_terminated(self, frame: lowering.Frame):
        """Branch to the next block if the current block is not terminated.

        This must be used after exhausting the current frame and before popping the frame.
        """
        if not frame.curr_block.last_stmt or not frame.curr_block.last_stmt.has_trait(
            ir.IsTerminator
        ):
            frame.curr_block.stmts.append(
                cf.Branch(arguments=(), successor=frame.next_block)
            )

    def current_block_terminated(self, frame: lowering.Frame):
        return frame.curr_block.last_stmt and frame.curr_block.last_stmt.has_trait(
            ir.IsTerminator
        )
