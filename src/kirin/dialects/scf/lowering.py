import ast

from kirin import ir, lowering
from kirin.exceptions import DialectLoweringError

from .stmts import Yield, IfElse
from ._dialect import dialect


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_If(self, state: lowering.LoweringState, node: ast.If) -> lowering.Result:
        cond = state.visit(node.test).expect_one()
        body_frame = lowering.Frame.from_stmts(node.body, state)
        body_frame.defs.update(state.current_frame.defs)
        state.push_frame(body_frame)
        state.exhaust(body_frame)

        def yield_callback(frame: lowering.Frame, value: ir.SSAValue) -> ir.SSAValue:
            pass

        ir.ResultValue
        after_frame = lowering.Frame.from_stmts(
            state.current_frame.stream.split(), state
        )
        state.push_frame(after_frame)
        # body_frame = self._frame_body(frame, state, cond, node.body)
        # if node.orelse:
        #     else_frame = self._frame_body(frame, state, cond, node.orelse)
        # else:
        #     else_frame = None

        # frame_after = self.scan_after(state, frame)
        # for value in frame_after.captures:
        #     pass

        # entry_block = frame_after.current_region.blocks[0]
        # body_yield: list[ir.SSAValue] = []
        # used_args: list[ir.SSAValue] = []
        # for arg in entry_block.args:
        #     if arg.name is None:
        #         continue
        #     value = body_frame.get_local(arg.name)
        #     if value is None:
        #         raise DialectLoweringError(
        #             f"undefined value in body branch for {arg.name}"
        #         )
        #     body_yield.append(value)
        #     used_args.append(arg)

        # self.append_yield(body_frame, body_yield)

        # if else_frame:
        #     else_yield: list[ir.SSAValue] = []
        #     for arg in entry_block.args:
        #         if arg.name is None:
        #             continue
        #         value = else_frame.get_local(arg.name)
        #         if not isinstance(value, ir.SSAValue):
        #             raise DialectLoweringError(
        #                 f"undefined value in else branch for {arg.name}"
        #             )
        #         else_yield.append(value)
        #     self.append_yield(else_frame, else_yield)

        # if_stmt = state.append_stmt(
        #     IfElse(
        #         cond,
        #         body_frame.current_region,
        #         else_frame.current_region if else_frame else ir.Region(),
        #     )
        # )
        # for arg, value in zip(used_args, if_stmt.results):
        #     arg.replace_by(value)

        # for stmt in entry_block.stmts:
        #     stmt.detach()
        #     frame.append_stmt(stmt)

        # for block in frame_after.current_region.blocks[1:]:
        #     block.detach()
        #     frame.append_block(block)
        # return lowering.Result()

    def _frame_body(
        self,
        frame: lowering.Frame,
        state: lowering.LoweringState,
        cond: ir.SSAValue,
        body: list[ast.stmt],
    ):
        body_region = ir.Region()
        body_frame = state.push_frame(
            lowering.Frame.from_stmts(
                body, state, region=body_region, globals=frame.globals
            )
        )
        if cond.name:
            body_frame.defs[cond.name] = cond
        body_frame.next_block = frame.next_block
        state.exhaust(body_frame)
        state.pop_frame()
        return body_frame

    def scan_phi(self, defs: dict[str, set[ir.SSAValue]], frame: lowering.Frame):
        for name, values in frame.defs.items():
            phi = defs.setdefault(name, set())
            if isinstance(values, set):
                phi.update(values)
            else:
                phi.add(values)

    def scan_after(
        self,
        state: lowering.LoweringState,
        frame: lowering.Frame,
        defs: dict[str, set[ir.SSAValue]],
    ):
        frame.defs.update(defs)
        frame_after = state.push_frame(
            lowering.Frame.from_stmts(
                frame.stream.split(),
                state,
                globals=frame.globals,
            )
        )
        frame_after.next_block = frame.next_block
        state.exhaust(frame_after)
        return state.pop_frame()

    def append_yield(self, frame: lowering.Frame, yield_values: list[ir.SSAValue]):
        if frame.current_block.last_stmt and frame.current_block.last_stmt.has_trait(
            ir.IsTerminator
        ):
            return
        else:
            frame.current_block.stmts.append(Yield(*yield_values))
