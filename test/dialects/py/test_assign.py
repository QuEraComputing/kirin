from typing import Literal

from kirin import types
from kirin.prelude import basic, basic_no_opt
from kirin.analysis import TypeInference
from kirin.dialects import py, func, ilist


@basic_no_opt
def main(x):
    y: int = x
    return y


def test_ann_assign():
    stmt = main.callable_region.blocks[0].stmts.at(0)
    assert isinstance(stmt, py.assign.TypeAssert)

    typeinfer = TypeInference(basic_no_opt)
    _, ret = typeinfer.run(main, types.Int)
    assert ret.is_equal(types.Int)
    _, ret = typeinfer.run(main, types.Float)
    assert ret is ret.bottom()


def test_typeinfer_simplify_assert():
    @basic(typeinfer=True, fold=False)
    def simplify(x: int):
        y: int = x
        return y

    stmt = simplify.callable_region.blocks[0].stmts.at(0)
    assert isinstance(stmt, func.Return)


def test_list_assign():
    @basic_no_opt.add(ilist)
    def list_assign():
        xs: ilist.IList[float, Literal[3]] = ilist.IList([1, 2, 3], elem=types.Float)
        return xs

    stmt = list_assign.callable_region.blocks[0].stmts.at(3)
    assert isinstance(stmt, ilist.New)
    assert stmt.elem_type.is_equal(types.Float)
    assert stmt.result.type.is_equal(ilist.IListType[types.Float, types.Literal(3)])

    stmt = list_assign.callable_region.blocks[0].stmts.at(4)
    assert isinstance(stmt, py.assign.TypeAssert)
    assert stmt.expected.is_equal(ilist.IListType[types.Float, types.Literal(3)])
