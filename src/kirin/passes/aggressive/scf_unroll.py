from dataclasses import field, dataclass

from kirin import ir, rewrite
from kirin.passes import Pass
from kirin.rewrite import abc
from kirin.passes.typeinfer import TypeInfer
from kirin.dialects.scf.unroll import ForLoop, PickIfElse

from ..fold import Fold


@dataclass
class UnrollScf(Pass):
    """This pass can be used to unroll scf.For loops and inline/expand scf.IfElse when
    the input are known at compile time.

    """

    typeinfer: TypeInfer = field(init=False)
    fold: Fold = field(init=False)

    def __post_init__(self):
        self.typeinfer = TypeInfer(self.dialects, no_raise=self.no_raise)
        self.fold = Fold(self.dialects, no_raise=self.no_raise)

    def unsafe_run(self, mt: ir.Method):
        result = abc.RewriteResult()
        result = rewrite.Walk(PickIfElse()).rewrite(mt.code).join(result)
        result = rewrite.Walk(ForLoop()).rewrite(mt.code).join(result)
        result = self.typeinfer(mt).join(result)
        result = self.fold(mt).join(result)
        return result
