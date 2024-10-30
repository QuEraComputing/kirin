from dataclasses import dataclass, field

from kirin import ir
from kirin.analysis.dataflow.constprop import Const, ConstPropLattice
from kirin.dialects import cf, func
from kirin.dialects.py import stmts
from kirin.rewrite import RewriteResult, RewriteRule


@dataclass
class ConstantFold(RewriteRule):
    results: dict[ir.SSAValue, ConstPropLattice] = field(default_factory=dict)

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if node.has_trait(ir.ConstantLike):
            return RewriteResult()
        elif isinstance(node, cf.ConditionalBranch):
            return self.rewrite_cf_ConditionalBranch(node)

        all_constants = True
        has_done_something = False
        for old_result in node.results:
            if isinstance(value := self.results.get(old_result, None), Const):
                stmt = stmts.Constant(value.data)
                stmt.insert_before(node)
                old_result.replace_by(stmt.result)
                if old_result.name:
                    stmt.result.name = old_result.name
                has_done_something = True
            else:
                all_constants = False

        # TODO: generalize func.Call to anything similar to call
        # NOTE: if we find call prop a const, depsite it is pure or not
        # the constant call only executes a pure branch of the code
        # thus it is safe to delete the call
        if all_constants and (node.has_trait(ir.Pure) or isinstance(node, func.Invoke)):
            node.delete()
        return RewriteResult(has_done_something=has_done_something)

    def rewrite_cf_ConditionalBranch(self, node: cf.ConditionalBranch):
        if isinstance(value := self.results.get(node.cond, None), Const):
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
