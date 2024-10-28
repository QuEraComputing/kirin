from dataclasses import dataclass

from kirin import ir
from kirin.analysis.dataflow.constprop import Const, ConstPropLattice
from kirin.dialects.func import Call, Invoke
from kirin.rewrite import RewriteResult, RewriteRule


@dataclass
class Call2Invoke(RewriteRule):
    """Rewrite a `Call` statement to an `Invoke` statement."""

    results: dict[ir.SSAValue, ConstPropLattice]

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, Call):
            return RewriteResult()

        if (mt := self.results.get(node.callee)) is None:
            return RewriteResult()

        if not isinstance(mt, Const):
            return RewriteResult()

        stmt = Invoke(inputs=node.inputs, callee=mt.data, kwargs=node.kwargs)
        stmt.result.name = node.result.name
        stmt.result.type = node.result.type
        node.replace_by(stmt)
        return RewriteResult(has_done_something=True)
