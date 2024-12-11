from typing import Iterable

from kirin import exceptions, interp, ir
from kirin.analysis.forward import Forward

from .lattice import Result, Value


class Propagate(Forward[Result]):
    keys = ["constprop", "empty"]
    lattice = Result

    def __init__(
        self,
        dialects: ir.DialectGroup | Iterable[ir.Dialect],
        *,
        fuel: int | None = None,
        max_depth: int = 128,
        max_python_recursion_depth: int = 8192,
    ):
        super().__init__(
            dialects,
            fuel=fuel,
            max_depth=max_depth,
            max_python_recursion_depth=max_python_recursion_depth,
        )
        self.interp = interp.Interpreter(
            dialects,
            fuel=fuel,
            max_depth=max_depth,
            max_python_recursion_depth=max_python_recursion_depth,
        )

    def try_eval_const(
        self, stmt: ir.Statement, args: tuple[Value, ...]
    ) -> interp.Result[Result]:
        try:
            value = self.interp.eval_stmt(stmt, tuple(x.data for x in args))
            if isinstance(value, interp.ResultValue):
                return interp.ResultValue(*tuple(Value(each) for each in value.values))
            elif isinstance(value, interp.ReturnValue):
                return interp.ReturnValue(Value(value.result))
            elif isinstance(value, interp.Successor):
                return interp.Successor(
                    value.block, *tuple(Value(each) for each in value.block_args)
                )
        except exceptions.InterpreterError:
            pass
        return interp.ResultValue(self.bottom)

    def eval_stmt(self, stmt: ir.Statement, args: tuple) -> interp.Result[Result]:
        if stmt.has_trait(ir.ConstantLike) or (
            stmt.has_trait(ir.Pure) and all(isinstance(x, Value) for x in args)
        ):
            return self.try_eval_const(stmt, args)

        sig = self.build_signature(stmt, args)
        if sig in self.registry:
            return self.registry[sig](self, stmt, args)
        elif stmt.__class__ in self.registry:
            return self.registry[stmt.__class__](self, stmt, args)
        else:
            # fallback to NotConst for other pure statements
            return interp.ResultValue(self.bottom.top())

    def run_method_region(
        self, mt: ir.Method, body: ir.Region, args: tuple[Result, ...]
    ) -> Result:
        return self.run_ssacfg_region(body, (Value(mt),) + args)
