from kirin.prelude import structural
from kirin.dialects import ilist
from kirin import types

def test_infer_lambda():
    @structural(typeinfer=True, fold=False, no_raise=False)
    def main(n):
        def map_func(i):
            return n + 1
        
        return ilist.map(map_func, ilist.range(4))

    map_stmt = main.callable_region.blocks[0].stmts.at(-2)
    assert isinstance(map_stmt, ilist.Map)
    assert map_stmt.result.type == ilist.IListType[types.Int, types.Literal(4)]

