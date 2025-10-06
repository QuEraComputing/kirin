from dataclasses import field, dataclass

from kirin.ir import Method
from kirin.passes import Fold, Pass, TypeInfer
from kirin.rewrite import Walk
from kirin.rewrite.abc import RewriteResult
from kirin.dialects.scf.unroll import ForLoop, PickIfElse


@dataclass
class UnrollScf(Pass):
    """This pass can be used to unroll scf.For loops and inline/expand scf.IfElse when
    the input are known at compile time.

    usage:
        UnrollScf(dialects).fixpoint(method)

    Note: This pass should be used in a fixpoint manner, to unroll nested scf nodes.

    """

    typeinfer: TypeInfer = field(init=False)
    fold: Fold = field(init=False)

    def __post_init__(self):
        self.typeinfer = TypeInfer(self.dialects, no_raise=self.no_raise)
        self.fold = Fold(self.dialects, no_raise=self.no_raise)

    def unsafe_run(self, mt: Method):
        result = RewriteResult()
        result = Walk(PickIfElse()).rewrite(mt.code).join(result)
        result = Walk(ForLoop()).rewrite(mt.code).join(result)
        result = self.fold.unsafe_run(mt).join(result)
        self.typeinfer.unsafe_run(mt)
        return result
