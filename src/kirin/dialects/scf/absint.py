from kirin import ir, interp
from kirin.analysis import const
from kirin.dialects import func

from .stmts import Yield, IfElse
from ._dialect import dialect


@dialect.register(key="absint")
class Methods(interp.MethodTable):

    @interp.impl(Yield)
    def yield_stmt(
        self,
        interp_: interp.AbstractInterpreter,
        frame: interp.AbstractFrame,
        stmt: Yield,
    ):
        return interp.YieldValue(frame.get_values(stmt.values))

    @interp.impl(IfElse)
    def if_else(
        self,
        interp_: interp.AbstractInterpreter,
        frame: interp.AbstractFrame,
        stmt: IfElse,
    ):
        if isinstance(hint := stmt.cond.hints.get("const"), const.Value):
            if hint.data:
                return self._infer_if_else_cond(interp_, frame, stmt, stmt.then_body)
            else:
                return self._infer_if_else_cond(interp_, frame, stmt, stmt.else_body)
        then_results = self._infer_if_else_cond(interp_, frame, stmt, stmt.then_body)
        else_results = self._infer_if_else_cond(interp_, frame, stmt, stmt.else_body)

        match (then_results, else_results):
            case (interp.ReturnValue(then_value), interp.ReturnValue(else_value)):
                return interp.ReturnValue(then_value.join(else_value))
            case (interp.ReturnValue(then_value), _):
                return then_results
            case (_, interp.ReturnValue(else_value)):
                return else_results
            case _:
                return interp_.join_results(then_results, else_results)

    def _infer_if_else_cond(
        self,
        interp_: interp.AbstractInterpreter,
        frame: interp.AbstractFrame,
        stmt: IfElse,
        body: ir.Region,
    ):
        body_block = body.blocks[0]
        body_term = body_block.last_stmt
        if isinstance(body_term, func.Return):
            frame.worklist.append(interp.Successor(body_block, frame.get(stmt.cond)))
            return

        with interp_.state.new_frame(interp_.new_frame(stmt)) as body_frame:
            body_frame.entries.update(frame.entries)
            body_frame.set(body_block.args[0], frame.get(stmt.cond))
            ret = interp_.run_ssacfg_region(body_frame, body)
            frame.entries.update(body_frame.entries)
            return ret
