from dataclasses import field, dataclass

from kirin import ir
from kirin.analysis import const
from kirin.dialects import cf, func
from kirin.dialects.py import stmts
from kirin.rewrite.abc import RewriteRule, RewriteResult


@dataclass
class ConstantFold(RewriteRule):
    results: dict[ir.SSAValue, const.JointResult] = field(default_factory=dict)

    def get_const(self, value: ir.SSAValue):
        ret = self.results.get(value, None)
        if ret is not None and isinstance(ret.const, const.Value):
            return ret.const
        return None

    def delete_node(self, node: ir.Statement):
        node.delete()
        for result in node.results:
            self.results.pop(result, None)

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if node.has_trait(ir.ConstantLike):
            return RewriteResult()
        elif isinstance(node, cf.ConditionalBranch):
            return self.rewrite_cf_ConditionalBranch(node)

        all_constants = True
        has_done_something = False
        for old_result in node.results:
            if (value := self.get_const(old_result)) is not None:
                stmt = stmts.Constant(value.data)
                stmt.insert_before(node)
                old_result.replace_by(stmt.result)
                self.results[stmt.result] = self.results[old_result]
                if old_result.name:
                    stmt.result.name = old_result.name
                has_done_something = True
            else:
                all_constants = False

        # TODO: generalize func.Call to anything similar to call
        # NOTE: if we find call prop a const, depsite it is pure or not
        # the constant call only executes a pure branch of the code
        # thus it is safe to delete the call
        if all_constants and node.has_trait(ir.Pure):
            self.delete_node(node)

        if (
            all_constants
            and isinstance(node, func.Invoke)
            and (value := self.results.get(node.result, None)) is not None
            and isinstance(value.purity, const.Pure)
        ):
            self.delete_node(node)
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
