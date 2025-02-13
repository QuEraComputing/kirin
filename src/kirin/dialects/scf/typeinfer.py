from kirin import ir, types, interp
from kirin.analysis import ForwardFrame, TypeInference, const
from kirin.dialects import func
from kirin.dialects.eltype import ElType

from .stmts import For, Yield, IfElse
from ._dialect import dialect


@dialect.register(key="typeinfer")
class TypeInfer(interp.MethodTable):

    @interp.impl(Yield)
    def yield_stmt(
        self,
        interp_: TypeInference,
        frame: ForwardFrame[types.TypeAttribute],
        stmt: Yield,
    ):
        return interp.YieldValue(frame.get_values(stmt.values))

    @interp.impl(IfElse)
    def if_else(
        self,
        interp_: TypeInference,
        frame: ForwardFrame[types.TypeAttribute],
        stmt: IfElse,
    ):
        frame.set(
            stmt.cond, frame.get(stmt.cond).meet(types.Bool)
        )  # set cond backwards
        if isinstance(hint := stmt.cond.hints.get("const"), const.Value):
            if hint.data:
                return self._infer_if_else_cond(interp_, frame, stmt, stmt.then_body)
            else:
                return self._infer_if_else_cond(interp_, frame, stmt, stmt.else_body)
        then_results = self._infer_if_else_cond(interp_, frame, stmt, stmt.then_body)
        else_results = self._infer_if_else_cond(interp_, frame, stmt, stmt.else_body)
        return interp_.join_results(then_results, else_results)

    def _infer_if_else_cond(
        self,
        interp_: TypeInference,
        frame: ForwardFrame[types.TypeAttribute],
        stmt: IfElse,
        body: ir.Region,
    ):
        body_block = body.blocks[0]
        body_term = body_block.last_stmt
        if isinstance(body_term, func.Return):  # TODO: use types.Literal?
            frame.worklist.append(interp.Successor(body_block, types.Bool))
            return

        with interp_.state.new_frame(interp_.new_frame(stmt)) as body_frame:
            body_frame.entries.update(frame.entries)
            body_frame.set(body_block.args[0], types.Bool)
            return interp_.run_ssacfg_region(body_frame, stmt.then_body)

    @interp.impl(For)
    def for_loop(
        self,
        interp_: TypeInference,
        frame: ForwardFrame[types.TypeAttribute],
        stmt: For,
    ):
        iterable = frame.get(stmt.iterable)
        loop_vars = frame.get_values(stmt.initializers)
        body_block = stmt.body.blocks[0]
        block_args = body_block.args

        eltype = interp_.run_stmt(ElType(ir.TestValue()), (iterable,))
        if not isinstance(eltype, tuple):  # error
            return (interp_.lattice.bottom(),)
        item = eltype[0]
        frame.set_values(block_args, (item,) + loop_vars)

        if isinstance(body_block.last_stmt, func.Return):
            frame.worklist.append(interp.Successor(body_block, item, *loop_vars))
            return  # if terminate is Return, there is no result

        loop_vars_ = interp_.run_ssacfg_region(frame, stmt.body)
        if isinstance(loop_vars_, interp.ReturnValue):
            return loop_vars_
        elif isinstance(loop_vars_, tuple):
            return interp_.join_results(loop_vars, loop_vars_)
        else:  # None, loop has no result
            return
