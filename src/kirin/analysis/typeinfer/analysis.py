from typing import TypeGuard, final

from kirin import ir, types, interp
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

    # NOTE: unlike concrete interpreter, instead of using type information
    # within the IR. Type inference will use the interpreted
    # value (which is a type) to determine the method dispatch.
    def build_signature(
        self, frame: ForwardFrame[types.TypeAttribute, None], stmt: ir.Statement
    ) -> Signature:
        _args = ()
        for x in frame.get_values(stmt.args):
            _args += (self._unwrap(x),)
        return Signature(stmt.__class__, _args)

    def _unwrap(self, value: types.TypeAttribute) -> types.TypeAttribute:
        if isinstance(value, types.Hinted):
            return self._unwrap(value.type)
        elif isinstance(value, types.Generic):
            return value.body
        return value

    def eval_stmt_fallback(
        self, frame: ForwardFrame[types.TypeAttribute, None], stmt: ir.Statement
    ) -> tuple[types.TypeAttribute, ...] | interp.SpecialValue[types.TypeAttribute]:
        resolve = TypeResolution()
        for arg, value in zip(stmt.args, frame.get_values(stmt.args)):
            resolve.solve(arg.type, value)
        return tuple(resolve.substitute(result.type) for result in stmt.results)

    def run_method(
        self, method: ir.Method, args: tuple[types.TypeAttribute, ...]
    ) -> types.TypeAttribute:
        return self.run_callable(
            method.code, (types.Hinted(types.PyClass(ir.Method), method),) + args
        )

    def is_const(
        self, value: types.TypeAttribute
    ) -> TypeGuard[types.Hinted[const.Value]]:
        return isinstance(value, types.Hinted) and isinstance(value.data, const.Value)

    def is_partial_const(
        self, value: types.TypeAttribute
    ) -> TypeGuard[
        types.Hinted[const.Value]
        | types.Hinted[const.PartialTuple]
        | types.Hinted[const.PartialLambda]
    ]:
        return isinstance(value, types.Hinted) and isinstance(
            value.data, (const.Value, const.PartialConst)
        )
