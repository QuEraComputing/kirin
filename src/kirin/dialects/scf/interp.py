from kirin import interp

from .stmts import For, Yield, IfElse
from ._dialect import dialect


@dialect.register
class Concrete(interp.MethodTable):

    @interp.impl(Yield)
    def yield_stmt(self, interp_: interp.Interpreter, frame: interp.Frame, stmt: Yield):
        return interp.ReturnValue(*frame.get_values(stmt.values))

    @interp.impl(IfElse)
    def if_else(self, interp: interp.Interpreter, frame: interp.Frame, stmt: IfElse):
        cond = frame.get(stmt.cond)
        if cond:
            body = stmt.then_body
        else:
            body = stmt.else_body
        return interp.run_ssacfg_region(frame, body)

    @interp.impl(For)
    def for_loop(self, interpreter: interp.Interpreter, frame: interp.Frame, stmt: For):
        iterable = frame.get(stmt.iterable)
        loop_vars = frame.get_values(stmt.initializers)
        block = stmt.body.blocks[0]
        # NOTE: we have checked this is always a Yield
        yield_stmt: Yield = block.last_stmt  # type: ignore

        for value in iterable:
            frame.set_values(block.args, (value,) + loop_vars)

            for each_stmt in block.stmts:
                if isinstance(each_stmt, Yield):
                    loop_vars = frame.get_values(yield_stmt.values)
                    break

                result = interpreter.eval_stmt(frame, each_stmt)
                if isinstance(result, interp.ReturnValue):
                    return result
                elif isinstance(result, tuple):
                    frame.set_values(each_stmt.results, result)
                else:
                    raise interp.InterpreterError(f"unexpected result: {result}")

        return loop_vars
