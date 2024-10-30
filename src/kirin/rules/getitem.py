from dataclasses import dataclass

from kirin import ir
from kirin.analysis.dataflow.constprop import Const, ConstPropLattice
from kirin.dialects.py import stmts
from kirin.rewrite import RewriteResult, RewriteRule


@dataclass
class InlineGetItem(RewriteRule):
    results: dict[ir.SSAValue, ConstPropLattice]

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, stmts.GetItem):
            return RewriteResult()

        if not isinstance(node.obj.owner, stmts.NewTuple):
            return RewriteResult()

        if node.index not in self.results:
            return RewriteResult()

        if not isinstance(index_value := self.results[node.index], Const):
            return RewriteResult()

        stmt = node.obj.owner
        index = index_value.data
        if isinstance(index, int) and 0 <= index < len(stmt.args):
            node.result.replace_by(stmt.args[index])
            return RewriteResult(has_done_something=True)
        elif isinstance(index, slice):
            start, stop, step = index.indices(len(stmt.args))
            new_tuple = stmts.NewTuple(
                tuple(stmt.args[start:stop:step]),
            )
            node.replace_by(new_tuple)
            return RewriteResult(has_done_something=True)
        else:
            return RewriteResult()
