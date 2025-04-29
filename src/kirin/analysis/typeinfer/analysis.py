from typing import TypeVar, final

from kirin import ir, types, interp
from kirin.decl import fields
from kirin.analysis import const
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

    keys = ("typeinfer",)
    lattice = types.TypeAttribute

    def run(self, method: ir.Method, *args, **kwargs):
        if not args and not kwargs: # no args or kwargs
            # use the method signature to get the args
            args = method.arg_types
        return super().run(method, *args, **kwargs)

    def method_self(self, method: ir.Method) -> types.TypeAttribute:
        return method.self_type

    def eval_fallback(
        self, frame: ForwardFrame[types.TypeAttribute], node: ir.Statement
    ) -> interp.StatementResult[types.TypeAttribute]:
        resolve = TypeResolution()
        fs = fields(node)
        for f, value in zip(fs.args.values(), frame.get_values(node.args)):
            resolve.solve(f.type, value)
        for arg, f in zip(node.args, fs.args.values()):
            frame.set(arg, frame.get(arg).meet(resolve.substitute(f.type)))
        return tuple(resolve.substitute(result.type) for result in node.results)

    # NOTE: unlike concrete interpreter, instead of using type information
    # within the IR. Type inference will use the interpreted
    # value (which is a type) to determine the method dispatch.
    def build_signature(
        self, frame: ForwardFrame[types.TypeAttribute], node: ir.Statement
    ) -> interp.Signature:
        argtypes = ()
        for x in frame.get_values(node.args):
            if isinstance(x, types.Generic):
                argtypes += (x.body,)
            else:
                argtypes += (x,)
        return interp.Signature(type(node), argtypes)

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
