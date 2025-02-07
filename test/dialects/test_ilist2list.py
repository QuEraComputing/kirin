from kirin import ir
from kirin.prelude import python_basic
from kirin.dialects import func, ilist, lowering


@ir.dialect_group(python_basic.union([func, ilist, lowering.func]))
def basic_desugar(self):
    ilist_desugar = ilist.IListDesugar(self)

    def run_pass(
        mt: ir.Method,
    ) -> None:
        ilist_desugar(mt)

    return run_pass


def test_ilist2list_rewrite():

    x = [1, 2, 3, 4]

    @basic_desugar
    def ilist2_list():
        return x

    ilist2_list.print()

    x = ilist2_list()

    assert isinstance(x, ilist.IList)
