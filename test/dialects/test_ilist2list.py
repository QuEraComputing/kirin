from typing import Literal

from kirin import ir
from kirin.passes import aggressive
from kirin.prelude import python_basic
from kirin.dialects import func, ilist
from kirin.passes.typeinfer import TypeInfer


@ir.dialect_group(python_basic.union([func, ilist]))
def basic_desugar(self):
    aggressive_fold_pass = aggressive.Fold(self)
    typeinfer_pass = TypeInfer(self)
    ilist_desugar = ilist.IListDesugar(self)

    def run_pass(
        mt: ir.Method,
    ) -> None:
        ilist_desugar(mt)
        # aggressive_fold_pass.fixpoint(mt)
        # rewrite.Fixpoint(rewrite.Walk(ilist.rewrite.RewriteHinted())).rewrite(mt.code)
        # typeinfer_pass(mt)

    return run_pass


def test_ilist2list_rewrite():

    x = [1, 2, 3, 4]

    @basic_desugar
    def ilist2_list():
        return x

    ilist2_list.print()


test_ilist2list_rewrite()
