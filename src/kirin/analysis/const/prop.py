from typing import Iterable

from kirin import exceptions, interp, ir
from kirin.analysis.forward import Forward

from .lattice import JointResult, NotPure, Pure, Value


class Propagate(Forward[JointResult]):
    keys = ["constprop", "empty"]
    lattice = JointResult

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

    def _try_eval_const_pure(
        self, stmt: ir.Statement, args: tuple[Value, ...]
    ) -> interp.Result[JointResult]:
        try:
            value = self.interp.eval_stmt(stmt, tuple(x.data for x in args))
            if isinstance(value, interp.ResultValue):
                return interp.ResultValue(
                    *tuple(JointResult(Value(each), Pure()) for each in value.values)
                )
            elif isinstance(value, interp.ReturnValue):
                return interp.ReturnValue(JointResult(Value(value.result), Pure()))
            elif isinstance(value, interp.Successor):
                return interp.Successor(
                    value.block,
                    *tuple(
                        JointResult(Value(each), Pure()) for each in value.block_args
                    ),
                )
        except exceptions.InterpreterError:
            pass
        return interp.ResultValue(self.bottom)

    def eval_stmt(
        self, stmt: ir.Statement, args: tuple[JointResult, ...]
    ) -> interp.Result[JointResult]:
        if stmt.has_trait(ir.ConstantLike):
            return self._try_eval_const_pure(stmt, ())
        elif stmt.has_trait(ir.Pure):
            values = tuple(x.const for x in args)
            if ir.types.is_tuple_of(values, Value):
                return self._try_eval_const_pure(stmt, values)

        sig = self.build_signature(stmt, args)
        if sig in self.registry:
            return self.registry[sig](self, stmt, args)
        elif stmt.__class__ in self.registry:
            return self.registry[stmt.__class__](self, stmt, args)
        else:
            # fallback to NotConst for other pure statements
            return interp.ResultValue(self.bottom.top())

    def run_method_region(
        self, mt: ir.Method, body: ir.Region, args: tuple[JointResult, ...]
    ) -> JointResult:
        if len(self.state.frames) < self.max_depth:
            return self.run_ssacfg_region(
                body, (JointResult(Value(mt), NotPure()),) + args
            )
        return self.bottom
