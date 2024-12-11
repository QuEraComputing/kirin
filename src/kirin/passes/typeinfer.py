from dataclasses import dataclass

from kirin.analysis.typeinfer import TypeInference
from kirin.ir import Method
from kirin.passes.abc import Pass
from kirin.rewrite import RewriteResult, Walk
from kirin.rules.apply_type import ApplyType


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
