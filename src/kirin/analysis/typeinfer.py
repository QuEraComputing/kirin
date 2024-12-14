from kirin import ir, interp
from kirin.ir import types
from kirin.ir.method import Method
from kirin.interp.impl import Signature
from kirin.ir.nodes.stmt import Statement
from kirin.ir.nodes.region import Region
from kirin.analysis.forward import Forward, ForwardFrame


class TypeInference(Forward[types.TypeAttribute]):
    keys = ["typeinfer"]
    lattice = types.TypeAttribute

    # NOTE: unlike concrete interpreter, instead of using type information
    # within the IR. Type inference will use the interpreted
    # value (which is a type) to determine the method dispatch.
    def build_signature(
        self, frame: ForwardFrame[types.TypeAttribute, None], stmt: Statement
    ) -> Signature:
        _args = ()
        for x in frame.get_values(stmt.args):
            if isinstance(x, types.Const):
                _args += (x.typ,)
            elif isinstance(x, types.Generic):
                _args += (x.body,)
            else:
                _args += (x,)
        return Signature(stmt.__class__, _args)

    def eval_stmt(
        self, frame: ForwardFrame[types.TypeAttribute, None], stmt: Statement
    ) -> interp.Result[types.TypeAttribute]:
        method = self.lookup_registry(frame, stmt)
        if method is not None:
            return method(self, frame, stmt)
        return tuple(result.type for result in stmt.results)

    def run_method_region(
        self, mt: Method, body: Region, args: tuple[types.TypeAttribute, ...]
    ) -> types.TypeAttribute:
        if len(self.state.frames) < self.max_depth:
            # NOTE: widen method type here
            return self.run_ssacfg_region(
                body, (types.Const(mt, types.PyClass(ir.Method)),) + args
            )
        return types.Bottom
