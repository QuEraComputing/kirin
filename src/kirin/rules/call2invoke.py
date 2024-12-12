from dataclasses import dataclass

from kirin import ir
from kirin.analysis import const
from kirin.dialects.func import Call, Invoke
from kirin.rewrite import RewriteResult, RewriteRule


@dataclass
class Call2Invoke(RewriteRule):
    """Rewrite a `Call` statement to an `Invoke` statement."""

    results: dict[ir.SSAValue, const.JointResult]

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, Call):
            return RewriteResult()

        if (mt := self.results.get(node.callee)) is None:
            return RewriteResult()

        if not isinstance(mt.const, const.Value):
            return RewriteResult()

        stmt = Invoke(inputs=node.inputs, callee=mt.const.data, kwargs=node.kwargs)
        for result, new_result in zip(node.results, stmt.results):
            new_result.name = result.name
            new_result.type = result.type
            if result in self.results:
                self.results[new_result] = self.results.pop(result)

        node.replace_by(stmt)
        return RewriteResult(has_done_something=True)
