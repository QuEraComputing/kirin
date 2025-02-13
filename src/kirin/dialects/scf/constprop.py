from collections.abc import Iterable

from kirin import ir, interp
from kirin.analysis import const

from .stmts import For, Yield, IfElse
from ._dialect import dialect

# NOTE: unlike concrete interpreter, we need to use a new frame
# for each iteration because otherwise join two constant values
# will result in bottom (error) element.


@dialect.register(key="constprop")
class DialectConstProp(interp.MethodTable):

    @interp.impl(Yield)
    def yield_stmt(
        self,
        interp_: const.Propagate,
        frame: const.Frame,
        stmt: Yield,
    ):
        return interp.YieldValue(frame.get_values(stmt.values))

    @interp.impl(IfElse)
    def if_else(
        self,
        interp_: const.Propagate,
        frame: const.Frame,
        stmt: IfElse,
    ):
        cond = frame.get(stmt.cond)
        if isinstance(cond, const.Value):
            if cond.data:
                body = stmt.then_body
            else:
                body = stmt.else_body
            body_frame, ret = self._prop_const_cond_ifelse(
                interp_, frame, stmt, cond, body
            )
            frame.entries.update(body_frame.entries)
            return ret
        else:
            then_frame, then_results = self._prop_const_cond_ifelse(
                interp_, frame, stmt, const.Value(True), stmt.then_body
            )
            else_frame, else_results = self._prop_const_cond_ifelse(
                interp_, frame, stmt, const.Value(False), stmt.else_body
            )
            ret = interp_.join_results(then_results, else_results)

            if not then_frame.frame_is_not_pure or not else_frame.frame_is_not_pure:
                frame.should_be_pure.add(stmt)

            # NOTE: then_frame and else_frame do not change
            # parent frame variables value except cond
            frame.entries.update(then_frame.entries)
            frame.entries.update(else_frame.entries)
            frame.set(stmt.cond, cond)
        return ret

    def _prop_const_cond_ifelse(
        self,
        interp_: const.Propagate,
        frame: const.Frame,
        stmt: IfElse,
        cond: const.Value,
        body: ir.Region,
    ):
        with interp_.state.new_frame(interp_.new_frame(stmt)) as body_frame:
            body_frame.entries.update(frame.entries)
            body_frame.set(body.blocks[0].args[0], cond)
            results = interp_.run_ssacfg_region(body_frame, body)

        if not body_frame.frame_is_not_pure:
            frame.should_be_pure.add(stmt)
        return body_frame, results

    @interp.impl(For)
    def for_loop(
        self,
        interp_: const.Propagate,
        frame: const.Frame,
        stmt: For,
    ):
        iterable = frame.get(stmt.iterable)
        if isinstance(iterable, const.Value):
            return self._prop_const_iterable_forloop(interp_, frame, stmt, iterable)
        else:  # TODO: support other iteration
            return tuple(interp_.lattice.top() for _ in stmt.results)

    def _prop_const_iterable_forloop(
        self,
        interp_: const.Propagate,
        frame: const.Frame,
        stmt: For,
        iterable: const.Value,
    ):
        frame_is_not_pure = False
        if not isinstance(iterable.data, Iterable):
            raise interp.InterpreterError(
                f"Expected iterable, got {type(iterable.data)}"
            )

        loop_vars = frame.get_values(stmt.initializers)
        body_block = stmt.body.blocks[0]
        block_args = body_block.args

        for value in iterable.data:
            with interp_.state.new_frame(interp_.new_frame(stmt)) as body_frame:
                body_frame.entries.update(frame.entries)
                body_frame.set_values(
                    block_args,
                    (const.Value(value),) + loop_vars,
                )
                loop_vars = interp_.run_ssacfg_region(body_frame, stmt.body)

            if body_frame.frame_is_not_pure:
                frame_is_not_pure = True
            if loop_vars is None:
                loop_vars = ()
            elif isinstance(loop_vars, interp.ReturnValue):
                return loop_vars

        if not frame_is_not_pure:
            frame.should_be_pure.add(stmt)
        return loop_vars
