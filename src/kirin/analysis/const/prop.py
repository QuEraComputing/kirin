from typing import Iterable
from dataclasses import dataclass

from kirin import ir, interp, exceptions
from kirin.analysis.forward import ForwardExtra

from .lattice import Pure, Value, NotPure, Unknown, JointResult


@dataclass
class ExtraFrameInfo:
    frame_is_not_pure: bool = False


class Propagate(ForwardExtra[JointResult, ExtraFrameInfo]):
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
            if isinstance(value, tuple):
                return tuple(JointResult(Value(each), Pure()) for each in value)
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
        return (self.bottom,)

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
            ret = self.registry[sig](self, stmt, args)
            self._set_frame_not_pure(ret)
            return ret
        elif stmt.__class__ in self.registry:
            ret = self.registry[stmt.__class__](self, stmt, args)
            self._set_frame_not_pure(ret)
            return ret
        elif stmt.has_trait(ir.Pure):
            # fallback to top for other statements
            return (JointResult(Unknown(), Pure()),)
        else:
            return (JointResult(Unknown(), NotPure()),)

    def _set_frame_not_pure(self, result: interp.Result[JointResult]):
        frame = self.state.current_frame()
        if isinstance(result, tuple) and all(x.purity is Pure() for x in result):
            return

        if isinstance(result, interp.ReturnValue) and result.result.purity is Pure():
            return

        if isinstance(result, interp.Successor) and all(
            x.purity is Pure() for x in result.block_args
        ):
            return

        if frame.extra is None:
            frame.extra = ExtraFrameInfo(True)

    def run_method_region(
        self, mt: ir.Method, body: ir.Region, args: tuple[JointResult, ...]
    ) -> JointResult:
        if len(self.state.frames) >= self.max_depth:
            return self.bottom

        ret = self.run_ssacfg_region(body, (JointResult(Value(mt), NotPure()),) + args)
        frame = self.state.current_frame()
        if frame.extra is not None and frame.extra.frame_is_not_pure:
            return JointResult(ret.const, NotPure())
        return ret
