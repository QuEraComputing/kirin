from kirin import ir
from kirin.analysis.dataflow.forward import Forward
from kirin.dialects.py import types
from kirin.interp.base import InterpResult
from kirin.ir import TypeAttribute
from kirin.ir.method import Method
from kirin.ir.nodes.region import Region
from kirin.ir.nodes.stmt import Statement


class TypeInference(Forward[TypeAttribute]):
    keys = ["typeinfer", "typeinfer.default"]
    lattice = TypeAttribute

    def build_signature(self, stmt: Statement, args: tuple):
        """we use value here as signature as they will be types"""
        _args = []
        for x in args:
            if isinstance(x, types.PyConst):
                _args.append(x.typ)
            elif isinstance(x, types.PyGeneric):
                _args.append(x.body)
            else:
                _args.append(x)

        return (
            stmt.__class__,
            tuple(_args),
        )

    def run_method_region(
        self, mt: Method, body: Region, args: tuple[TypeAttribute, ...]
    ) -> InterpResult[TypeAttribute]:
        # NOTE: widen method type here
        return self.run_ssacfg_region(
            body, (types.PyConst(mt, types.PyClass(ir.Method)),) + args
        )
