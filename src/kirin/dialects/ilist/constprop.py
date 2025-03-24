from kirin import ir
from kirin.interp import MethodTable, impl
from kirin.analysis import const

from .stmts import Map, Scan, Foldl, Foldr, ForEach
from ._dialect import dialect


@dialect.register(key="constprop")
class ConstPropMethods(MethodTable):

    def detect_purity(
        self,
        constprop: const.Propagate,
        frame: const.Frame,
        stmt: ir.Statement,
        method_code: ir.Statement,
        args: tuple[const.Result, ...],
    ):
        method_frame, _ = constprop.run_callable(method_code, args)
        if not method_frame.frame_is_not_pure:
            frame.should_be_pure.add(stmt)

    @impl(Map)
    @impl(ForEach)
    def one_args(self, interp_: const.Propagate, frame: const.Frame, stmt: Map):
        fn, collection = frame.get(stmt.fn), frame.get(stmt.collection)

        # 1. if the function is a constant method, and the method is pure, then the map is pure
        if isinstance(fn, const.Value) and isinstance(method := fn.data, ir.Method):
            self.detect_purity(interp_, frame, stmt, method.code, (fn, const.Unknown()))
            if isinstance(collection, const.Value):
                return interp_.try_eval_const_pure(frame, stmt, (fn, collection))
        elif isinstance(fn, const.PartialLambda):
            self.detect_purity(interp_, frame, stmt, fn.code, (fn, const.Unknown()))

        return (const.Unknown(),)

    @impl(Foldl)
    @impl(Foldr)
    @impl(Scan)
    def two_args(self, interp_: const.Propagate, frame: const.Frame, stmt: Foldl):
        fn, collection, init = (
            frame.get(stmt.fn),
            frame.get(stmt.collection),
            frame.get(stmt.init),
        )

        # 1. if the function is a constant method, and the method is pure, then the foldl is pure
        if isinstance(fn, const.Value) and isinstance(method := fn.data, ir.Method):
            self.detect_purity(
                interp_,
                frame,
                stmt,
                method.code,
                (fn, const.Unknown(), const.Unknown()),
            )
            if isinstance(collection, const.Value) and isinstance(init, const.Value):
                return interp_.try_eval_const_pure(frame, stmt, (fn, collection, init))
        elif isinstance(fn, const.PartialLambda):
            self.detect_purity(
                interp_, frame, stmt, fn.code, (fn, const.Unknown(), const.Unknown())
            )

        return (const.Unknown(),)
