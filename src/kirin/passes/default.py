from dataclasses import field, dataclass

from kirin.ir.method import Method
from kirin.passes.fold import Fold
from kirin.rewrite.abc import RewriteResult
from kirin.passes.aggressive import Fold as AggressiveFold

from .abc import Pass
from .typeinfer import TypeInfer
from .hint_const import HintConst
from .canonicalize import Canonicalize


@dataclass
class Default(Pass):
    verify: bool = field(default=True, kw_only=True)
    fold: bool = field(default=True, kw_only=True)
    aggressive: bool = field(default=False, kw_only=True)
    typeinfer: bool = field(default=True, kw_only=True)

    canonicalize: Canonicalize = field(init=False)
    hint_const_pass: HintConst = field(init=False)
    typeinfer_pass: TypeInfer = field(init=False)
    fold_pass: Pass = field(init=False)

    def __post_init__(self):
        # TODO: cleanup no_raise
        self.typeinfer_pass = TypeInfer(self.dialects, no_raise=self.no_raise)
        self.canonicalize = Canonicalize(self.dialects, no_raise=self.no_raise)

        self.hint_const_pass = HintConst(self.dialects, no_raise=self.no_raise)
        if self.aggressive:
            self.fold_pass = AggressiveFold(self.dialects, no_raise=self.no_raise)
        else:
            self.fold_pass = Fold(self.dialects, no_raise=self.no_raise)

    def unsafe_run(self, mt: Method) -> RewriteResult:
        if self.verify:
            mt.verify()

        result = self.canonicalize.fixpoint(mt)
        if self.typeinfer:
            result = self.typeinfer_pass(mt).join(result)
            if self.verify:
                mt.verify_type()

        if self.fold:
            if self.aggressive:
                result = self.fold_pass.fixpoint(mt).join(result)
            else:
                result = self.fold_pass(mt).join(result)
        return result
