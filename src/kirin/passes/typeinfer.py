from dataclasses import field, dataclass

from kirin.ir import Method, HasSignature
from kirin.rewrite import Walk, Chain
from kirin.passes.abc import Pass
from kirin.rewrite.abc import RewriteResult
from kirin.dialects.func import Signature
from kirin.analysis.typeinfer import TypeInference
from kirin.rewrite.apply_type import ApplyType
from kirin.rewrite.type_assert import InlineTypeAssert

from .hint_const import HintConst
from .post_inference import PostInference


@dataclass
class TypeInfer(Pass):
    hint_const: HintConst = field(init=False)
    inference: PostInference = field(init=False)

    def __post_init__(self):
        self.infer = TypeInference(self.dialects)
        self.hint_const = HintConst(self.dialects, no_raise=self.no_raise)
        self.post_inference = PostInference(self.dialects, no_raise=self.no_raise)

    def unsafe_run(self, mt: Method) -> RewriteResult:
        result = self.hint_const.unsafe_run(mt)
        if self.no_raise:
            frame, return_type = self.infer.run_no_raise(mt, *mt.arg_types)
        else:
            frame, return_type = self.infer.run(mt, *mt.arg_types)

        if trait := mt.code.get_trait(HasSignature):
            trait.set_signature(mt.code, Signature(mt.arg_types, return_type))

        result = (
            Chain(
                Walk(ApplyType(frame.entries)),
                Walk(InlineTypeAssert()),
            )
            .rewrite(mt.code)
            .join(result)
        )
        result = self.post_inference.fixpoint(mt).join(result)
        mt.inferred = True
        return result
