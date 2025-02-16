from typing import Hashable
from dataclasses import field, dataclass

from kirin import ir
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.dialects.py.constant import Constant


@dataclass
class GlobalValueElimination(RewriteRule):
    """Rewrite to eliminate dupliacte `Constant` statements if its the same constant value.

    Note:
        This rule only works for hashable constant values.

    """

    seen: dict[int, ir.SSAValue] = field(default_factory=dict)

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:

        if not isinstance(node, Constant):
            return RewriteResult()

        # if constant value is not hashable, skip
        if not isinstance(node.value, Hashable):
            return RewriteResult()

        # get hash:
        hash_value: int = hash(node.value)

        if hash_value in self.seen:
            node.result.replace_by(self.seen[hash_value])
            return RewriteResult(has_done_something=True)
        else:
            self.seen[hash_value] = node.result
            return RewriteResult()
