from kirin import interp
from kirin.analysis import ForwardFrame, const

from .stmts import For, Yield, IfElse
from ._dialect import dialect


@dialect.register(key="constprop")
class DialectConstProp(interp.MethodTable):

    @interp.impl(Yield)
    def yield_stmt(
        self,
        interp_: const.Propagate,
        frame: interp.Frame[const.JointResult],
        stmt: Yield,
    ):
        return interp.YieldValue(frame.get_values(stmt.values))

    @interp.impl(IfElse)
    def if_else(
        self,
        interp_: const.Propagate,
        frame: ForwardFrame[const.JointResult, const.ExtraFrameInfo],
        stmt: IfElse,
    ):
        cond = frame.get(stmt.cond)
        if isinstance(cond.const, const.Value):
            if cond.const.data:
                results = interp_.run_ssacfg_region(frame, stmt.then_body)
            else:
                results = interp_.run_ssacfg_region(frame, stmt.else_body)
        else:
            results = interp_.join_results(
                interp_.run_ssacfg_region(frame, stmt.then_body),
                interp_.run_ssacfg_region(frame, stmt.else_body),
            )

        return results

    @interp.impl(For)
    def for_loop(
        self,
        interp_: const.Propagate,
        frame: ForwardFrame[const.JointResult, const.ExtraFrameInfo],
        stmt: For,
    ):
        iterable = frame.get(stmt.iterable)
        loop_vars = frame.get_values(stmt.initializers)
        block_args = stmt.body.blocks[0].args

        if isinstance(iterable.const, const.Value):
            for value in iterable.const.data:
                frame.set_values(
                    block_args,
                    (const.JointResult(const.Value(value), iterable.purity),)
                    + loop_vars,
                )
                loop_vars = interp_.run_ssacfg_region(frame, stmt.body)
                if loop_vars is None:
                    loop_vars = ()
                elif isinstance(loop_vars, interp.ReturnValue):
                    return loop_vars
            return loop_vars
        else:  # TODO: support other iteration
            return tuple(interp_.lattice.top() for _ in stmt.results)
