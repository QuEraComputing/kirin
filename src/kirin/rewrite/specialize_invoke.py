"""Rewrite static calls using a pass-owned specialization factory."""

from typing import Mapping, Callable
from dataclasses import dataclass

from kirin import ir
from kirin.analysis import const
from kirin.dialects import func
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.dialects.py.constant import Constant


@dataclass
class SpecializeInvoke(RewriteRule):
    """Specialize Invokes using fresh constant facts and a shared factory.

    The factory returns a method and the input positions it retains, or None
    when specialization is unsupported or exceeds its budget.
    """

    facts: Mapping[ir.SSAValue, const.Result]
    specialize: Callable[
        [ir.Method, tuple[const.Result, ...]],
        tuple[ir.Method, tuple[int, ...]] | None,
    ]

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, func.Invoke):
            return RewriteResult()
        args: list[const.Result] = []
        for value in node.inputs:
            if isinstance(value.owner, Constant) and isinstance(
                value.owner.value, ir.PyAttr
            ):
                args.append(const.Value(value.owner.value.data))
            else:
                args.append(self.facts.get(value, const.Unknown()))
        specialized = self.specialize(node.callee, tuple(args))
        if specialized is None:
            return RewriteResult()
        method, retained = specialized
        replacement = func.Invoke(
            callee=method,
            inputs=tuple(node.inputs[i] for i in retained),
            purity=node.purity,
        )
        replacement.source = node.source
        replacement.result.name = node.result.name
        replacement.result.type = node.result.type
        node.replace_by(replacement)
        return RewriteResult(has_done_something=True)
