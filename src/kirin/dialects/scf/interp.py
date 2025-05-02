from kirin import interp

from .stmts import For, Yield, IfElse
from ._dialect import dialect


@dialect.register
class Concrete(interp.MethodTable):

    @interp.impl(Yield)
    def yield_stmt(self, interp_: interp.Interpreter, frame: interp.Frame, stmt: Yield):
        return interp.YieldValue(frame.get_values(stmt.values))

    @interp.impl(IfElse)
    def if_else(self, interp_: interp.Interpreter, frame: interp.Frame, stmt: IfElse):
        cond = frame.get(stmt.cond)
        if cond:
            body = stmt.then_body
        else:
            body = stmt.else_body
        return interp_.frame_call_region(frame, stmt, body, cond)

    @interp.impl(For)
    def for_loop(self, interp_: interp.Interpreter, frame: interp.Frame, stmt: For):
        iterable = frame.get(stmt.iterable)
        loop_vars = frame.get_values(stmt.initializers)
        for value in iterable:
            loop_vars = interp_.frame_call_region(
                frame, stmt, stmt.body, value, *loop_vars
            )
            if isinstance(loop_vars, interp.ReturnValue):
                return loop_vars
            elif loop_vars is None:
                loop_vars = ()
        return loop_vars
