from kirin.dialects import cf, func
from kirin.dialects.py import data, stmts
from kirin.ir import types
from kirin.lowering import Lowering

lowering = Lowering([cf, func, data, stmts])


def test_empty_list():

    def empty_list():
        x = []
        return x

    code = lowering.run(empty_list)

    list_stmt = code.body.blocks[0].stmts.at(0)  # type: ignore

    assert isinstance(list_stmt, stmts.NewList)
    assert len(list_stmt._results) == 1

    res = list_stmt._results[0]
    assert res.type.is_subseteq(types.List)
