from dataclasses import dataclass

from kirin import ir
from kirin.analysis import const
from kirin.dialects import cf
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.dialects.py.constant import Constant


@dataclass
class ConstantFold(RewriteRule):

    def get_const(self, value: ir.SSAValue):
        ret = value.hints.get("const")

        if ret is not None and isinstance(ret, const.Value):
            return ret
        return None

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if node.has_trait(ir.ConstantLike):
            return RewriteResult()
        elif isinstance(node, cf.ConditionalBranch):
            return self.rewrite_cf_ConditionalBranch(node)

        if not self.is_pure(node):
            return RewriteResult()

        has_done_something = False
        for old_result in node.results:
            if (value := self.get_const(old_result)) is not None:
                stmt = Constant(value.data)
                stmt.insert_before(node)
                old_result.replace_by(stmt.result)
                stmt.result.hints["const"] = value
                if old_result.name:
                    stmt.result.name = old_result.name
                has_done_something = True
        return RewriteResult(has_done_something=has_done_something)

    def rewrite_cf_ConditionalBranch(self, node: cf.ConditionalBranch):
        if (value := self.get_const(node.cond)) is not None:
            if value.data is True:
                cf.Branch(
                    arguments=node.then_arguments,
                    successor=node.then_successor,
                ).insert_before(node)
            elif value.data is False:
                cf.Branch(
                    arguments=node.else_arguments,
                    successor=node.else_successor,
                ).insert_before(node)
            else:
                raise ValueError(f"Invalid constant value for branch: {value.data}")
            node.delete()
            return RewriteResult(has_done_something=True)
        return RewriteResult()
