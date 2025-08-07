from collections.abc import Iterable

from kirin import interp
from kirin.analysis import const
from kirin.dialects import func

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

            with interp_.new_frame(stmt, has_parent_access=True) as body_frame:
                ret = interp_.frame_call_region(body_frame, stmt, body, cond)
            frame.entries.update(body_frame.entries)

            if not body_frame.frame_is_not_pure and not isinstance(
                body.blocks[0].last_stmt, func.Return
            ):
                frame.should_be_pure.add(stmt)
            return ret
        else:
            with interp_.new_frame(stmt, has_parent_access=True) as then_frame:
                then_results = interp_.frame_call_region(
                    then_frame, stmt, stmt.then_body, const.Value(True)
                )

            with interp_.new_frame(stmt, has_parent_access=True) as else_frame:
                else_results = interp_.frame_call_region(
                    else_frame, stmt, stmt.else_body, const.Value(False)
                )

            # NOTE: then_frame and else_frame do not change
            # parent frame variables value except cond
            frame.entries.update(then_frame.entries)
            frame.entries.update(else_frame.entries)
            # TODO: pick the non-return value
            if isinstance(then_results, interp.ReturnValue) and isinstance(
                else_results, interp.ReturnValue
            ):
                return interp.ReturnValue(then_results.value.join(else_results.value))
            elif isinstance(then_results, interp.ReturnValue):
                ret = else_results
            elif isinstance(else_results, interp.ReturnValue):
                ret = then_results
            else:
                if not (
                    then_frame.frame_is_not_pure is True
                    or else_frame.frame_is_not_pure is True
                ):
                    frame.should_be_pure.add(stmt)
                ret = interp_.join_results(then_results, else_results)
        return ret

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

        for value in iterable.data:
            with interp_.new_frame(stmt, has_parent_access=True) as body_frame:
                loop_vars = interp_.frame_call_region(
                    body_frame, stmt, stmt.body, const.Value(value), *loop_vars
                )

            if body_frame.frame_is_not_pure:
                frame_is_not_pure = True
            if loop_vars is None:
                loop_vars = ()
            elif isinstance(loop_vars, interp.ReturnValue):
                return loop_vars

        if not frame_is_not_pure:
            frame.should_be_pure.add(stmt)
        return loop_vars
