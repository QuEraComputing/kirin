import ast

from kirin import ir, lowering
from kirin.dialects.py import stmts as pystmts, types

from . import stmts
from .dialect import dialect


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_For(
        self, state: lowering.LoweringState, node: ast.For
    ) -> lowering.Result:
        if node.orelse:
            raise NotImplementedError("for-else is not supported")

        iter_value = state.visit(node.iter).expect_one()
        iterator = state.append_stmt(stmts.Iter(iter_value)).result

        entry_block, defs = ir.Block(), {}
        item = entry_block.args.append_from(types.Any, "item")

        def callback(frame: lowering.Frame, value: ir.SSAValue):
            return entry_block.args.append_from(value.type, value.name)

        if isinstance(node.target, ast.Name):
            item.name = node.target.id
            defs[node.target.id] = item
        elif isinstance(node.target, ast.Tuple):
            # TODO: handle tuple unpacking
            raise NotImplementedError("Tuple unpacking is not supported")

        loop_frame = state.push_frame(
            lowering.Frame.from_stmts(
                node.body,
                state,
                globals=state.current_frame.globals,
                block=entry_block,
                capture_callback=callback,
            )
        )
        loop_frame.defs.update(defs)
        yield_block = ir.Block()
        loop_frame.next_block = yield_block
        state.exhaust(loop_frame)
        loop_frame.append_block(loop_frame.next_block)

        _loop_variables: list[ir.SSAValue] = []
        for name in loop_frame.captures.keys():
            if (value := loop_frame.get_local(name)) is not None:
                _loop_variables.append(value)
        loop_variables = tuple(_loop_variables)
        yield_block.stmts.append(stmts.Yield(loop_variables))

        state.pop_frame()
        for block in loop_frame.current_region.blocks:
            if block.last_stmt is None or not block.last_stmt.has_trait(
                ir.IsTerminator
            ):
                block.stmts.append(stmts.Yield(loop_variables))

        result_for = state.append_stmt(
            stmts.For(iterator, body=loop_frame.current_region)
        ).result
        target_value = state.append_stmt(
            pystmts.GetItem(result_for, state.append_stmt(pystmts.Constant(0)).result)
        ).result

        if isinstance(node.target, ast.Name):
            target_value.name = node.target.id
            state.current_frame.defs[node.target.id] = target_value

        for idx, value in enumerate(loop_variables):
            assert value.name is not None, "Expected value to have a name"
            new_acc = state.append_stmt(
                pystmts.GetItem(
                    result_for, state.append_stmt(pystmts.Constant(idx + 1)).result
                )
            ).result
            new_acc.name = value.name
            state.current_frame.defs[value.name] = new_acc
        return lowering.Result()  # for has no value
