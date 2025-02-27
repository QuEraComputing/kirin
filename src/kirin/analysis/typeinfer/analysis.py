from typing import TypeVar, final

from kirin import ir, types, interp
from kirin.decl import fields
from kirin.analysis import const
from kirin.interp.impl import Signature
from kirin.analysis.forward import Forward, ForwardFrame

from .solve import TypeResolution


@final
class TypeInference(Forward[types.TypeAttribute]):
    """Type inference analysis for kirin.

    This analysis uses the forward dataflow analysis framework to infer the types of
    the IR. The analysis uses the type information within the IR to determine the
    method dispatch.

    The analysis will fallback to a type resolution algorithm if the type information
    is not available in the IR but the type information is available in the abstract
    values.
    """

    keys = ["typeinfer"]
    lattice = types.TypeAttribute

    def run_analysis(
        self, method: ir.Method, args: tuple[types.TypeAttribute, ...] | None = None
    ) -> tuple[ForwardFrame[types.TypeAttribute], types.TypeAttribute]:
        if args is None:
            args = method.arg_types
        return super().run_analysis(method, args)

    # NOTE: unlike concrete interpreter, instead of using type information
    # within the IR. Type inference will use the interpreted
    # value (which is a type) to determine the method dispatch.
    def build_signature(
        self, frame: ForwardFrame[types.TypeAttribute], stmt: ir.Statement
    ) -> Signature:
        _args = ()
        for x in frame.get_values(stmt.args):
            # TODO: remove this after we have multiple dispatch...
            if isinstance(x, types.Generic):
                _args += (x.body,)
            else:
                _args += (x,)
        return Signature(stmt.__class__, _args)

    def eval_stmt_fallback(
        self, frame: ForwardFrame[types.TypeAttribute], stmt: ir.Statement
    ) -> tuple[types.TypeAttribute, ...] | interp.SpecialValue[types.TypeAttribute]:
        resolve = TypeResolution()
        fs = fields(stmt)
        for f, value in zip(fs.args.values(), frame.get_values(stmt.args)):
            resolve.solve(f.type, value)
        for arg, f in zip(stmt.args, fs.args.values()):
            frame.set(arg, frame.get(arg).meet(resolve.substitute(f.type)))
        return tuple(resolve.substitute(result.type) for result in stmt.results)

    def run_method(
        self, method: ir.Method, args: tuple[types.TypeAttribute, ...]
    ) -> tuple[ForwardFrame[types.TypeAttribute], types.TypeAttribute]:
        return self.run_callable(method.code, (method.self_type,) + args)

    T = TypeVar("T")

    @classmethod
    def maybe_const(cls, value: ir.SSAValue, type_: type[T]) -> T | None:
        """Get a constant value of a given type.

        If the value is not a constant or the constant is not of the given type, return
        `None`.
        """
        hint = value.hints.get("const")
        if isinstance(hint, const.Value) and isinstance(hint.data, type_):
            return hint.data

    @classmethod
    def expect_const(cls, value: ir.SSAValue, type_: type[T]):
        """Expect a constant value of a given type.

        If the value is not a constant or the constant is not of the given type, raise
        an `InterpreterError`.
        """
        hint = cls.maybe_const(value, type_)
        if hint is None:
            raise interp.InterpreterError(f"expected {type_}, got {hint}")
        return hint
