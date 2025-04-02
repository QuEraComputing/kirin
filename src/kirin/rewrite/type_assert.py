from dataclasses import dataclass

from kirin import ir
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.dialects.py.assign import TypeAssert


@dataclass
class InlineTypeAssert(RewriteRule):

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, TypeAssert):
            return RewriteResult()

        if node.got.type.is_subseteq(node.expected):
            node.got.type = node.got.type.meet(node.expected)
            node.result.replace_by(node.got)
            node.delete()
            return RewriteResult(has_done_something=True)
        return RewriteResult()
