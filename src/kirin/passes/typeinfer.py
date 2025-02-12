from dataclasses import dataclass

from kirin.ir import Method, HasSignature
from kirin.rewrite import Walk
from kirin.passes.abc import Pass
from kirin.rewrite.abc import RewriteResult
from kirin.dialects.func import Signature
from kirin.analysis.typeinfer import TypeInference
from kirin.rewrite.apply_type import ApplyType


@dataclass
class TypeInfer(Pass):

    def __post_init__(self):
        self.infer = TypeInference(self.dialects)

    def unsafe_run(self, mt: Method) -> RewriteResult:
        frame, return_type = self.infer.run_analysis(mt, mt.arg_types)
        if trait := mt.code.get_trait(HasSignature):
            trait.set_signature(mt.code, Signature(mt.arg_types, return_type))

        result = Walk(ApplyType(frame.entries)).rewrite(mt.code)
        mt.inferred = True
        return result
