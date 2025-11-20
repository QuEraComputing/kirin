from kirin import ir, types
from kirin.prelude import structural
from kirin.dialects import ilist


def test_infer_lambda():
    @structural(typeinfer=True, fold=False, no_raise=False)
    def main(n):
        def map_func(i):
            return n + 1

        return ilist.map(map_func, ilist.range(4))

    map_stmt = main.callable_region.blocks[0].stmts.at(-2)
    assert isinstance(map_stmt, ilist.Map)
    assert map_stmt.result.type == ilist.IListType[types.Int, types.Literal(4)]


def test_infer_method_type_hint_call():

    @structural(typeinfer=True, fold=False, no_raise=False)
    def main(n, fx: ir.Method[[int], int]):
        return fx(n)

    assert main.return_type == types.Int


def test_infer_method_type_hint():

    @structural(typeinfer=True, fold=False, no_raise=False)
    def main(n, fx: ir.Method[[int], int]):
        def map_func(i):
            return n + 1 + fx(i)

        return ilist.map(map_func, ilist.range(4))

    assert main.return_type == ilist.IListType[types.Int, types.Literal(4)]
