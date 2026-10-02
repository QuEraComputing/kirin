from dataclasses import dataclass

from kirin import ir
from kirin.decl import info, statement
from kirin.passes import Specialize
from kirin.prelude import basic_no_opt
from kirin.analysis import const
from kirin.dialects import py, func
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.rewrite.specialize_invoke import SpecializationFactory

dialect = ir.Dialect("specialize_extension")


@statement(dialect=dialect)
class CustomCall(ir.Statement):
    callee: ir.Method = info.attribute()
    inputs: tuple[ir.SSAValue, ...] = info.argument()
    result: ir.ResultValue = info.result()


@dataclass
class SpecializeCustomCall(RewriteRule):
    specialize: SpecializationFactory

    def rewrite_Statement(self, node):
        if not isinstance(node, CustomCall):
            return RewriteResult()
        args = tuple(value.hints.get("const", const.Unknown()) for value in node.inputs)
        if (variant := self.specialize(node.callee, args)) is None:
            return RewriteResult()
        method, retained = variant
        node.replace_by(
            CustomCall(callee=method, inputs=tuple(node.inputs[i] for i in retained))
        )
        return RewriteResult(has_done_something=True)


def test_custom_calls_share_materializer_and_queue():
    kernel = basic_no_opt.add(dialect)

    @kernel
    def helper(x: int, scale: int):
        return x + scale * 2

    @kernel
    def root(x: int):
        return helper(x, 3), helper(x, 3), helper(x, 3)

    # Mix DSL callsites with the built-in func.Invoke specialization.
    invokes = [node for node in root.code.walk() if isinstance(node, func.Invoke)]
    for node in invokes[:2]:
        node.replace_by(CustomCall(callee=node.callee, inputs=node.inputs))

    factories = []

    def factory(materialize):
        factories.append(materialize)
        return SpecializeCustomCall(materialize)

    pass_ = Specialize(root.dialects, extra_rules=(factory,), no_raise=False)
    pass_(root)
    calls = [node for node in root.code.walk() if isinstance(node, CustomCall)]
    assert len(calls) == 2
    variant = calls[0].callee
    assert variant is calls[1].callee and variant is not helper
    assert (
        next(s for s in root.code.walk() if isinstance(s, func.Invoke)).callee
        is variant
    )
    assert len(factories) == 1
    assert len(calls[0].inputs) == 1
    # Processing the queued clone folds the multiplication exposed by binding.
    assert not any(isinstance(node, py.binop.Mult) for node in variant.code.walk())
    assert variant(4) == helper(4, 3) == 10
    variant.verify()
    pass_(root)
    assert len(factories) == 2  # A fresh rule for each invocation of the pass.
