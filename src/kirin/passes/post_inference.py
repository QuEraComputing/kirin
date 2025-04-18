from dataclasses import dataclass

from kirin.ir.method import Method
from kirin.rewrite.abc import RewriteResult

from .abc import Pass


@dataclass
class PostInference(Pass):

    def unsafe_run(self, mt: Method) -> RewriteResult:
        result = RewriteResult()
        for dialect in self.dialects:
            for rule in dialect.rules.inference:
                result = rule.rewrite(mt.code).join(result)
        return result
