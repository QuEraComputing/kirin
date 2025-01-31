import ast

from kirin import ir, lowering
from kirin.exceptions import DialectLoweringError
from kirin.dialects.py.unpack import unpackable

from .stmts import For, Yield, IfElse
from ._dialect import dialect


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_If(self, state: lowering.LoweringState, node: ast.If) -> lowering.Result:
        cond = state.visit(node.test).expect_one()
        frame = state.current_frame
        body_frame = lowering.Frame.from_stmts(node.body, state)
        state.push_frame(body_frame)
        state.exhaust(body_frame)
        state.pop_frame()

        else_frame = lowering.Frame.from_stmts(node.orelse, state)
        state.push_frame(else_frame)
        state.exhaust(else_frame)
        state.pop_frame()

        yield_names: list[str] = []
        body_yields: list[ir.SSAValue] = []
        else_yields: list[ir.SSAValue] = []
        if node.orelse:
            for name in body_frame.defs.keys():
                if name in else_frame.defs:
                    yield_names.append(name)
                    body_yields.append(body_frame.get_scope(name))
                    else_yields.append(else_frame.get_scope(name))
        else:
            for name in body_frame.defs.keys():
                if name in frame.defs:
                    yield_names.append(name)
                    body_yields.append(body_frame.get_scope(name))
                    value = frame.get(name)
                    if value is None:
                        raise DialectLoweringError(f"expected value for {name}")
                    else_yields.append(value)

        body_frame.append_stmt(Yield(*body_yields))
        else_frame.append_stmt(Yield(*else_yields))
        stmt = IfElse(
            cond,
            then_body=body_frame.current_region,
            else_body=else_frame.current_region,
        )
        for result, name, body, else_ in zip(
            stmt.results, yield_names, body_yields, else_yields
        ):
            result.name = name
            result.type = body.type.join(else_.type)
            frame.defs[name] = result
        state.append_stmt(stmt)
        return lowering.Result()

    def lower_For(
        self, state: lowering.LoweringState, node: ast.For
    ) -> lowering.Result:
        iter_ = state.visit(node.iter).expect_one()

        yields: list[str] = []

        def new_block_arg_if_inside_loop(frame: lowering.Frame, capture: ir.SSAValue):
            if not capture.name:
                raise DialectLoweringError("unexpected loop variable captured")
            yields.append(capture.name)
            return frame.current_block.args.append_from(capture.type, capture.name)

        body_frame = state.push_frame(
            lowering.Frame.from_stmts(
                node.body, state, capture_callback=new_block_arg_if_inside_loop
            )
        )
        loop_var = body_frame.current_block.args.append_from(ir.types.Any)
        unpackable(state, node.target, loop_var)
        state.exhaust(body_frame)
        # NOTE: this frame won't have phi nodes
        body_frame.append_stmt(Yield(*[body_frame.defs[name] for name in yields]))  # type: ignore
        state.pop_frame()

        initializers: list[ir.SSAValue] = []
        for name in yields:
            value = state.current_frame.get(name)
            if value is None:
                raise DialectLoweringError(f"expected value for {name}")
            initializers.append(value)
        stmt = For(iter_, body_frame.current_region, *initializers)
        for name, result in zip(yields, stmt.results):
            state.current_frame.defs[name] = result
        state.append_stmt(stmt)
        return lowering.Result()
