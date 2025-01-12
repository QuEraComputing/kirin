from typing import Iterable

from kirin import ir, types, interp
from kirin.analysis import const
from kirin.interp.impl import Signature
from kirin.analysis.forward import Forward, ForwardFrame

from .solve import TypeResolution


class TypeInference(Forward[types.TypeAttribute]):
    keys = ["typeinfer"]
    lattice = types.TypeAttribute

    def __init__(
        self,
        dialects: ir.DialectGroup | Iterable[ir.Dialect],
        *,
        fuel: int | None = None,
        save_all_ssa: bool = False,
        max_depth: int = 128,
        max_python_recursion_depth: int = 8192,
    ):
        super().__init__(
            dialects,
            fuel=fuel,
            save_all_ssa=save_all_ssa,
            max_depth=max_depth,
            max_python_recursion_depth=max_python_recursion_depth,
        )
        self.constprop_results: dict[ir.SSAValue, const.JointResult] = {}

    # NOTE: unlike concrete interpreter, instead of using type information
    # within the IR. Type inference will use the interpreted
    # value (which is a type) to determine the method dispatch.
    def build_signature(
        self, frame: ForwardFrame[types.TypeAttribute, None], stmt: ir.Statement
    ) -> Signature:
        _args = ()
        for x in frame.get_values(stmt.args):
            if isinstance(x, types.Annotated):
                _args += (x.typ,)
            elif isinstance(x, types.Generic):
                _args += (x.body,)
            else:
                _args += (x,)
        return Signature(stmt.__class__, _args)

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
                method.code, (types.Annotated(method, types.PyClass(ir.Method)),) + args
            )
        return types.Bottom

    def unwrap_const(self, value: types.TypeAttribute) -> types.TypeAttribute:
        if isinstance(value, types.Annotated) and isinstance(value.data, const.Result):
            return value.typ
        return value
