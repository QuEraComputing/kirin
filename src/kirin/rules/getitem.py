from dataclasses import dataclass

from kirin import ir
from kirin.analysis.dataflow.constprop import Const, ConstPropLattice
from kirin.dialects.py import stmts, types
from kirin.rewrite import RewriteResult, RewriteRule


@dataclass
class InlineGetItem(RewriteRule):
    results: dict[ir.SSAValue, ConstPropLattice]

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, stmts.GetItem):
            return RewriteResult()

        if not node.obj.type.is_subtype(types.Tuple):
            return RewriteResult()

        if not node.index.type.is_subtype(types.Int):
            return RewriteResult()

        if not isinstance(node.obj.owner, stmts.NewTuple):
            return RewriteResult()

        if node.index not in self.results:
            return RewriteResult()

        if not isinstance(value := self.results[node.index], Const):
            return RewriteResult()

        stmt = node.obj.owner
        index: int = value.data
        node.result.replace_by(stmt.args[index])
        return RewriteResult(has_done_something=True)
