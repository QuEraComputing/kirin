from typing import TypeGuard
from dataclasses import dataclass

from kirin.ir import Block, SSAValue, Statement, types
from kirin.rewrite import RewriteRule, RewriteResult
from kirin.analysis import const


@dataclass
class WrapConst(RewriteRule):
    results: dict[SSAValue, const.JointResult]

    @staticmethod
    def worth_record(
        value: const.Result,
    ) -> TypeGuard[const.Value | const.PartialLambda | const.PartialTuple]:
        return isinstance(value, (const.Value, const.PartialLambda, const.PartialTuple))

    def wrap_result(self, value: SSAValue):
        if isinstance(value.type, types.Annotated) and isinstance(
            value.type.data, const.Result
        ):
            # already annotated, skip
            return False

        if (arg_result := self.results.get(value)) is not None and self.worth_record(
            arg_result.const
        ):
            value.type = types.Annotated(arg_result.const, value.type)
            return True

        return False

    def rewrite_Block(self, node: Block) -> RewriteResult:
        has_done_something = False
        for arg in node.args:
            if self.wrap_result(arg):
                has_done_something = True
        return RewriteResult(has_done_something=has_done_something)

    def rewrite_Statement(self, node: Statement) -> RewriteResult:
        has_done_something = False
        for result in node._results:
            if self.wrap_result(result):
                has_done_something = True
        return RewriteResult(has_done_something=has_done_something)
