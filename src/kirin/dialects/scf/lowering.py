import ast

from kirin import ir, lowering
from kirin.exceptions import DialectLoweringError
from kirin.dialects.py import stmts as pystmts, types

from . import stmts
from .dialect import dialect


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_If(self, state: lowering.LoweringState, node: ast.If) -> lowering.Result:
        frame_body = state.push_frame(
            lowering.Frame.from_stmts(
                node.body, state, globals=state.current_frame.globals
            )
        )
        state.exhaust(frame_body)
        state.pop_frame()

        frame_else = state.push_frame(
            lowering.Frame.from_stmts(
                node.orelse, state, globals=state.current_frame.globals
            )
        )
        state.exhaust(frame_else)
        state.pop_frame()

        body_yield_values: dict[str, ir.SSAValue] = {}
        else_yield_values: dict[str, ir.SSAValue] = {}
        for name in frame_body.defs.keys():
            if (
                name in frame_else.defs
                and (then_value := frame_body.get_local(name)) is not None
                and (else_value := frame_else.get_local(name)) is not None
            ):
                body_yield_values[name] = then_value
                else_yield_values[name] = else_value
            elif (
                name in state.current_frame.defs
                and (then_value := frame_body.get_local(name)) is not None
                and (else_value := state.current_frame.get_local(name)) is not None
            ):
                body_yield_values[name] = then_value
                else_yield_values[name] = else_value
            else:  # body local variable
                continue

        for name in frame_else.defs.keys():
            # defined in else only, but have previous definition
            if (
                name in state.current_frame.defs
                and (then_value := state.current_frame.get_local(name)) is not None
                and (else_value := frame_else.get_local(name)) is not None
            ):
                body_yield_values[name] = then_value
                else_yield_values[name] = else_value
            else:  # body local variable
                continue

        then_block = frame_body.current_region.blocks[-1]
        else_block = frame_else.current_region.blocks[-1]

        if (
            then_block.last_stmt is not None
            and then_block.last_stmt.has_trait(ir.IsTerminator)
        ) and (
            else_block.last_stmt is None
            or not else_block.last_stmt.has_trait(ir.IsTerminator)
        ):  # then block terminates, all values in else defined
            state.current_frame.defs.update(frame_else.defs)
        elif (
            else_block.last_stmt is not None
            and else_block.last_stmt.has_trait(ir.IsTerminator)
        ) and (
            then_block.last_stmt is None
            or not then_block.last_stmt.has_trait(ir.IsTerminator)
        ):  # else block terminates, all values in then defined
            state.current_frame.defs.update(frame_body.defs)

        if (
            then_block.last_stmt is None
            or not then_block.last_stmt.has_trait(ir.IsTerminator)
            and body_yield_values
        ):
            then_block.stmts.append(stmts.Yield(tuple(body_yield_values.values())))

        if (
            else_block.last_stmt is None
            or not else_block.last_stmt.has_trait(ir.IsTerminator)
            and else_yield_values
        ):
            else_block.stmts.append(stmts.Yield(tuple(else_yield_values.values())))

        stmt = state.append_stmt(
            stmts.IfElse(
                state.visit(node.test).expect_one(),
                frame_body.current_region,
                frame_else.current_region,
            )
        )
        for name, value in zip(body_yield_values.keys(), stmt.results):
            state.current_frame.defs[name] = value
            value.name = name
        return lowering.Result()

    def lower_For(
        self, state: lowering.LoweringState, node: ast.For
    ) -> lowering.Result:
        if node.orelse:
            raise NotImplementedError("for-else is not supported")

        iter_value = state.visit(node.iter).expect_one()
        iterator = state.append_stmt(stmts.Iter(iter_value)).result
        body_frame = state.push_frame(lowering.Frame.from_stmts(node.body, state))
        body_block = body_frame.current_block
        item = body_block.args.append_from(types.Any, "item")

        if isinstance(node.target, ast.Name):
            item.name = node.target.id
            body_frame.defs[node.target.id] = item
        elif isinstance(node.target, ast.Tuple):
            # TODO: handle tuple unpacking
            raise NotImplementedError("Tuple unpacking is not supported")

        state.exhaust(body_frame)
        state.pop_frame()
        state.append_stmt(stmts.For(iterator, body_frame.current_region))
        return lowering.Result()  # for has no value
