"""Rewrite static calls using a pass-owned specialization factory."""

from typing import Callable
from dataclasses import dataclass

from kirin import ir
from kirin.analysis import const
from kirin.dialects import func
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.dialects.py.constant import Constant

SpecializationFactory = Callable[
    [ir.Method, tuple[const.Result, ...]],
    tuple[ir.Method, tuple[int, ...]] | None,
]
ClosureSpecializationFactory = Callable[
    [func.Lambda, tuple[const.Result, ...]],
    tuple[func.Lambda, tuple[int, ...]] | None,
]


def _input_facts(values: tuple[ir.SSAValue, ...]) -> tuple[const.Result, ...]:
    args: list[const.Result] = []
    for value in values:
        if isinstance(value.owner, Constant):
            args.append(const.Value(value.owner.value))
        else:
            hint = value.hints.get("const")
            args.append(hint if isinstance(hint, const.Result) else const.Unknown())
    return tuple(args)


@dataclass
class SpecializeInvoke(RewriteRule):
    """Specialize Invokes using constant hints and a shared factory.

    The factory returns a method and the input positions it retains, or None
    when specialization is unsupported or exceeds its budget.
    """

    specialize: SpecializationFactory

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, func.Invoke):
            return RewriteResult()
        specialized = self.specialize(node.callee, _input_facts(node.inputs))
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
        if hint := node.result.hints.get("const"):
            replacement.result.hints["const"] = hint
        node.replace_by(replacement)
        return RewriteResult(has_done_something=True)


@dataclass
class SpecializeClosureCall(RewriteRule):
    """Specialize calls to a local closure using constant hints and a factory.

    The callee must be the result of a `func.Lambda` statement, typically a
    closure with a capture known only at run time. The factory returns a clone
    of that statement, created with the same captures, and the input positions
    the clone retains, or None.
    """

    specialize: ClosureSpecializationFactory

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, func.Call) or node.kwargs:
            return RewriteResult()
        callee = node.callee
        if not (
            isinstance(callee, ir.ResultValue) and isinstance(callee.owner, func.Lambda)
        ):
            return RewriteResult()
        specialized = self.specialize(callee.owner, _input_facts(node.inputs))
        if specialized is None:
            return RewriteResult()
        clone, retained = specialized
        replacement = func.Call(
            clone.result,
            tuple(node.inputs[i] for i in retained),
            (),
            purity=node.purity,
        )
        replacement.source = node.source
        replacement.result.name = node.result.name
        replacement.result.type = node.result.type
        if hint := node.result.hints.get("const"):
            replacement.result.hints["const"] = hint
        node.replace_by(replacement)
        return RewriteResult(has_done_something=True)
