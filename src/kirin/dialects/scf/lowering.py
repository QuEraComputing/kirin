import ast

from kirin import ir, types, lowering
from kirin.dialects.py.unpack import unpacking

from .stmts import For, Yield, IfElse
from ._dialect import dialect


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_If(self, state: lowering.State, node: ast.If) -> lowering.Result:
        cond = state.lower(node.test).expect_one()
        frame = state.current_frame

        with state.frame(node.body, finalize_next=False) as body_frame:
            then_cond = body_frame.curr_block.args.append_from(types.Bool, cond.name)
            if cond.name:
                body_frame.defs[cond.name] = then_cond
            body_frame.exhaust()

        with state.frame(node.orelse, finalize_next=False) as else_frame:
            else_cond = else_frame.curr_block.args.append_from(types.Bool, cond.name)
            if cond.name:
                else_frame.defs[cond.name] = else_cond
            else_frame.exhaust()

        yield_names: list[str] = []
        body_yields: list[ir.SSAValue] = []
        else_yields: list[ir.SSAValue] = []
        if node.orelse:
            for name in body_frame.defs.keys():
                if name in else_frame.defs:
                    yield_names.append(name)
                    body_yields.append(body_frame[name])
                    else_yields.append(else_frame[name])
        else:
            for name in body_frame.defs.keys():
                if name in frame.defs:
                    yield_names.append(name)
                    body_yields.append(body_frame[name])
                    value = frame.get(name)
                    if value is None:
                        raise lowering.BuildError(f"expected value for {name}")
                    else_yields.append(value)

        if not (
            body_frame.curr_block.last_stmt
            and body_frame.curr_block.last_stmt.has_trait(ir.IsTerminator)
        ):
            body_frame.push(Yield(*body_yields))

        if not (
            else_frame.curr_block.last_stmt
            and else_frame.curr_block.last_stmt.has_trait(ir.IsTerminator)
        ):
            else_frame.push(Yield(*else_yields))

        stmt = IfElse(
            cond,
            then_body=body_frame.curr_region,
            else_body=else_frame.curr_region,
        )
        for result, name, body, else_ in zip(
            stmt.results, yield_names, body_yields, else_yields
        ):
            result.name = name
            result.type = body.type.join(else_.type)
            frame.defs[name] = result
        state.current_frame.push(stmt)

    def lower_For(self, state: lowering.State, node: ast.For) -> lowering.Result:
        iter_ = state.lower(node.iter).expect_one()

        yields: list[str] = []
        parent_frame = state.current_frame

        def new_block_arg_if_inside_loop(frame: lowering.Frame, capture: ir.SSAValue):
            if not capture.name:
                raise lowering.BuildError("unexpected loop variable captured")
            yields.append(capture.name)
            return frame.curr_block.args.append_from(capture.type, capture.name)

        with state.frame(
            node.body,
            capture_callback=new_block_arg_if_inside_loop,
            finalize_next=False,
        ) as body_frame:
            loop_var = body_frame.curr_block.args.append_from(types.Any)
            unpacking(state, node.target, loop_var)
            body_frame.exhaust()

            # if a variable is assigned in loop body and exist in parent frame
            # it should be captured as initializers and yielded
            for name, value in body_frame.defs.items():
                if name in parent_frame.defs:
                    yields.append(name)
                    body_frame.curr_block.args.append_from(value.type, name)

            # NOTE: this frame won't have phi nodes
            if yields and (
                body_frame.curr_block.last_stmt is None
                or not body_frame.curr_block.last_stmt.has_trait(ir.IsTerminator)
            ):
                body_frame.push(Yield(*[body_frame.defs[name] for name in yields]))  # type: ignore

        initializers: list[ir.SSAValue] = []
        for name in yields:
            value = state.current_frame.get(name)
            if value is None:
                raise lowering.BuildError(f"expected value for {name}")
            initializers.append(value)
        stmt = For(iter_, body_frame.curr_region, *initializers)

        for name, result in zip(yields, stmt.results):
            state.current_frame.defs[name] = result
            result.name = name
        state.current_frame.push(stmt)
