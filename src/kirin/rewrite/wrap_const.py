from dataclasses import dataclass

from kirin import ir
from kirin.analysis import const
from kirin.rewrite.abc import RewriteRule, RewriteResult


@dataclass
class WrapConst(RewriteRule):
    frame: const.Frame

    def wrap(self, value: ir.SSAValue) -> bool:
        result = self.frame.entries.get(value)
        if not result:
            return False

        const_hint = value.hints.get("const")
        if const_hint and isinstance(const_hint, const.Result):
            const_result = result.join(const_hint)
        else:
            const_result = result
        value.hints["const"] = const_result
        return True

    def rewrite_Block(self, node: ir.Block) -> RewriteResult:
        has_done_something = False
        for arg in node.args:
            if self.wrap(arg):
                has_done_something = True
        return RewriteResult(has_done_something=has_done_something)

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        has_done_something = False
        for result in node.results:
            if self.wrap(result):
                has_done_something = True

        if (
            trait := node.get_trait(ir.MaybePure)
        ) and node in self.frame.should_be_pure:
            trait.set_pure(node)
            has_done_something = True
        return RewriteResult(has_done_something=has_done_something)
