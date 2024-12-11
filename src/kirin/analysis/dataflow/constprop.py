from typing import Iterable

from kirin import exceptions, interp, ir

from .forward import Forward
from .lattice.const import Const, ConstLattice, NotConst


class ConstProp(Forward[ConstLattice]):
    keys = ["constprop", "empty"]
    lattice = ConstLattice

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
        self, stmt: ir.Statement, args: tuple[Const, ...]
    ) -> interp.Result[ConstLattice]:
        try:
            value = self.interp.eval_stmt(stmt, tuple(x.data for x in args))
            if isinstance(value, interp.ResultValue):
                return interp.ResultValue(*tuple(Const(each) for each in value.values))
            elif isinstance(value, interp.ReturnValue):
                return interp.ReturnValue(Const(value.result))
            elif isinstance(value, interp.Successor):
                return interp.Successor(
                    value.block, *tuple(Const(each) for each in value.block_args)
                )
        except exceptions.InterpreterError:
            pass
        return interp.ResultValue(NotConst())

    def eval_stmt(self, stmt: ir.Statement, args: tuple) -> interp.Result[ConstLattice]:
        if stmt.has_trait(ir.ConstantLike) or (
            stmt.has_trait(ir.Pure) and all(isinstance(x, Const) for x in args)
        ):
            return self.try_eval_const(stmt, args)

        sig = self.build_signature(stmt, args)
        if sig in self.registry:
            return self.registry[sig](self, stmt, args)
        elif stmt.__class__ in self.registry:
            return self.registry[stmt.__class__](self, stmt, args)
        else:
            # fallback to NotConst for other pure statements
            return interp.ResultValue(NotConst())

    def run_method_region(
        self, mt: ir.Method, body: ir.Region, args: tuple[ConstLattice, ...]
    ) -> ConstLattice:
        return self.run_ssacfg_region(body, (Const(mt),) + args)
