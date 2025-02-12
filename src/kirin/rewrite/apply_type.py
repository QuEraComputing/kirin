from dataclasses import dataclass

from kirin import ir, types
from kirin.rewrite.abc import RewriteRule, RewriteResult


@dataclass
class ApplyType(RewriteRule):
    results: dict[ir.SSAValue, types.TypeAttribute]

    def rewrite_Block(self, node: ir.Block) -> RewriteResult:
        has_done_something = False
        for arg in node.args:
            if arg in self.results:
                arg.type = self.results[arg]
                has_done_something = True

        return RewriteResult(has_done_something=has_done_something)

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        has_done_something = False
        for result in node._results:
            if result in self.results:
                result.type = self.results[result]
                has_done_something = True

        return RewriteResult(has_done_something=has_done_something)
