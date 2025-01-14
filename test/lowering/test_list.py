from kirin.ir import types
from kirin.dialects import cf, py, func
from kirin.lowering import Lowering

lowering = Lowering([cf, func, py.base, py.ilist, py.append, py.assign])


def test_empty_list():

    def empty_list():
        x = []
        return x

    code = lowering.run(empty_list)

    list_stmt = code.body.blocks[0].stmts.at(0)  # type: ignore

    assert isinstance(list_stmt, py.ilist.New)
    assert len(list_stmt._results) == 1

    res = list_stmt._results[0]
    assert res.type.is_subseteq(types.List)
