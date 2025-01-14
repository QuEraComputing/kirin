from dataclasses import dataclass

from kirin.ir import Method
from kirin.rewrite import Walk
from kirin.passes.abc import Pass
from kirin.rewrite.abc import RewriteResult
from kirin.analysis.typeinfer import TypeInference
from kirin.rewrite.apply_type import ApplyType


@dataclass
class TypeInfer(Pass):

    def __post_init__(self):
        self.infer = TypeInference(self.dialects)

    def unsafe_run(self, mt: Method) -> RewriteResult:
        return_type = self.infer.eval(mt, mt.arg_types).expect()
        mt.return_type = return_type
        mt.inferred = True
        result = Walk(ApplyType(return_type, self.infer.results)).rewrite(mt.code)
        self.infer.results.clear()
        return result
