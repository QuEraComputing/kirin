from typing import TypeGuard
from dataclasses import dataclass

from kirin import ir
from kirin.analysis import const
from kirin.rewrite.abc import RewriteRule, RewriteResult


@dataclass
class WrapConst(RewriteRule):
    results: dict[ir.SSAValue, const.JointResult]

    @staticmethod
    def worth(
        value: const.Result,
    ) -> TypeGuard[const.Value | const.PartialLambda | const.PartialTuple]:
        return isinstance(value, (const.Value, const.PartialLambda, const.PartialTuple))

    def wrap(self, value: ir.SSAValue) -> bool:
        if isinstance(value.type, ir.types.Hinted) and isinstance(
            value.type.data, const.Result
        ):
            return False

        if (result := self.results.get(value)) is not None and self.worth(result.const):
            value.type = ir.types.Hinted(value.type, result.const)
            return True

        return False

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
        return RewriteResult(has_done_something=has_done_something)
