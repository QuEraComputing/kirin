from kirin import ir
from kirin.rewrite import Walk
from kirin.passes.abc import Pass
from kirin.rewrite.abc import RewriteResult

from .rewrites.desugar import DesugarBinOp


class VMathDesugar(Pass):
    """This pass desugars the Python list dialect
    to the immutable list dialect by rewriting all
    constant `list` type into `IList` type.
    """

    def unsafe_run(self, mt: ir.Method) -> RewriteResult:
        return Walk(DesugarBinOp()).rewrite(mt.code)
