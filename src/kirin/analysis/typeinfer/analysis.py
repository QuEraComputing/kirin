from typing import TypeGuard

from kirin import ir, types, interp
from kirin.analysis import const
from kirin.interp.impl import Signature
from kirin.analysis.forward import Forward, ForwardFrame

from .solve import TypeResolution


class TypeInference(Forward[types.TypeAttribute]):
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

    def eval_stmt(
        self, frame: ForwardFrame[types.TypeAttribute, None], stmt: ir.Statement
    ) -> interp.StatementResult[types.TypeAttribute]:
        method = self.lookup_registry(frame, stmt)
        if method is not None:
            return method(self, frame, stmt)

        resolve = TypeResolution()
        for arg, value in zip(stmt.args, frame.get_values(stmt.args)):
            resolve.solve(arg.type, value)
        return tuple(resolve.substitute(result.type) for result in stmt.results)

    def run_method(
        self, method: ir.Method, args: tuple[types.TypeAttribute, ...]
    ) -> interp.MethodResult[types.TypeAttribute]:
        if len(self.state.frames) < self.max_depth:
            # NOTE: widen method type here
            return self.run_callable(
                method.code, (types.Hinted(types.PyClass(ir.Method), method),) + args
            )
        return types.Bottom

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
