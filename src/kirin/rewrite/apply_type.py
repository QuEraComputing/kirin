from dataclasses import dataclass

from kirin import ir, types
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.dialects.func.attrs import Signature


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

        if (trait := node.get_trait(ir.HasSignature)) is not None and (
            callable_trait := node.get_trait(ir.CallableStmtInterface)
        ) is not None:
            callable_region = callable_trait.get_callable_region(node)
            inputs = tuple(
                self.results.get(arg, arg.type)
                for arg in callable_region.blocks[0].args
            )

            if (
                len(node._results) == 1
                and isinstance(
                    output_ := self.results.get(node._results[0]), types.Generic
                )
                and output_.is_subseteq(types.MethodType)
            ):
                output_ = output_.vars[1]
                trait.set_signature(node, Signature(inputs, output_))
                has_done_something = True
        return RewriteResult(has_done_something=has_done_something)
