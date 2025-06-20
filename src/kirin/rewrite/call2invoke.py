from dataclasses import dataclass

from kirin import ir
from kirin.analysis import const
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.dialects.func import Call, Invoke


@dataclass
class Call2Invoke(RewriteRule):
    """Rewrite a `Call` statement to an `Invoke` statement."""

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, Call):
            return RewriteResult()

        if (mt := node.callee.hints.get("const")) is None:
            return RewriteResult()

        if not isinstance(mt, const.Value):
            return RewriteResult()

        if not isinstance(mt.data, ir.Method):
            return RewriteResult()

        method = mt.data
        trait = method.code.get_present_trait(ir.CallableStmtInterface)
        inputs = trait.align_input_args(
            method.code, *node.inputs, **dict(zip(node.keys, node.kwargs))
        )
        stmt = Invoke(inputs=inputs, callee=mt.data)
        for result, new_result in zip(node.results, stmt.results):
            new_result.name = result.name
            new_result.type = result.type
            if result_hint := result.hints.get("const"):
                new_result.hints["const"] = result_hint

        node.replace_by(stmt)
        return RewriteResult(has_done_something=True)
