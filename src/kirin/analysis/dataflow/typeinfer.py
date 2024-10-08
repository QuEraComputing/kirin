from dataclasses import dataclass

from kirin.analysis.dataflow.forward import ForwardDataFlowAnalysis
from kirin.dialects.py import types
from kirin.interp import Successor
from kirin.ir import BottomType, TypeAttribute
from kirin.ir.nodes.stmt import Statement
from kirin.worklist import WorkList


@dataclass(init=False)
class TypeInference(ForwardDataFlowAnalysis[TypeAttribute, WorkList[Successor]]):
    keys = ["typeinfer", "typeinfer.default"]

    @classmethod
    def bottom_value(cls) -> TypeAttribute:
        return BottomType()

    @classmethod
    def default_worklist(cls) -> WorkList[Successor]:
        return WorkList()

    def build_signature(self, stmt: Statement, args: tuple):
        """we use value here as signature as they will be types"""
        return (
            stmt.__class__,
            tuple(x.typ if isinstance(x, types.PyConst) else x for x in args),
        )
