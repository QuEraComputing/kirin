from dataclasses import dataclass

from kirin.ir import Block, SSAValue, Statement, types
from kirin.rewrite.abc import RewriteRule, RewriteResult


@dataclass
class ApplyType(RewriteRule):
    results: dict[SSAValue, types.TypeAttribute]

    def rewrite_Block(self, node: Block) -> RewriteResult:
        has_done_something = False
        for arg in node.args:
            if arg in self.results:
                arg.type = self.results[arg]
                has_done_something = True

        return RewriteResult(has_done_something=has_done_something)

    def rewrite_Statement(self, node: Statement) -> RewriteResult:
        has_done_something = False
        for result in node._results:
            if result in self.results:
                result.type = self.results[result]
                has_done_something = True

        return RewriteResult(has_done_something=has_done_something)
