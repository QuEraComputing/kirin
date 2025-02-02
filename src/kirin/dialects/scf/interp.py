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
        return interp_.run_ssacfg_region(frame, body)

    @interp.impl(For)
    def for_loop(self, interpreter: interp.Interpreter, frame: interp.Frame, stmt: For):
        iterable = frame.get(stmt.iterable)
        loop_vars = frame.get_values(stmt.initializers)
        block_args = stmt.body.blocks[0].args
        for value in iterable:
            frame.set_values(block_args, (value,) + loop_vars)
            loop_vars = interpreter.run_ssacfg_region(frame, stmt.body)
            if isinstance(loop_vars, interp.ReturnValue):
                return loop_vars
            elif loop_vars is None:
                loop_vars = ()
        return loop_vars
