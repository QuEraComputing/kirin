from typing import Any, Literal

from kirin import rewrite
from kirin.prelude import basic
from kirin.dialects import py, ilist


def test():
    rule = rewrite.Fixpoint(rewrite.Walk(py.len.InferLen()))

    @basic
    def len_func(xs: ilist.IList[int, Literal[3]]):
        return len(xs)

    @basic
    def len_func2(xs: tuple[int, float, int, float, int]):
        return len(xs)

    @basic
    def len_func3(xs: ilist.IList[int, Any]):
        return len(xs)

    @basic
    def len_func4(xs: tuple[int, ...]):
        return len(xs)

    rule.rewrite(len_func.code)
    rule.rewrite(len_func2.code)
    rule.rewrite(len_func3.code)
    rule.rewrite(len_func4.code)

    len_func.print()
    len_func2.print()
    len_func3.print()
    len_func4.print()
    stmt = len_func.callable_region.blocks[0].stmts.at(0)
    assert isinstance(stmt, py.Constant)
    assert stmt.value.unwrap() == 3

    stmt = len_func2.callable_region.blocks[0].stmts.at(0)
    assert isinstance(stmt, py.Constant)
    assert stmt.value.unwrap() == 5

    stmt = len_func3.callable_region.blocks[0].stmts.at(0)
    assert isinstance(stmt, py.Len)

    stmt = len_func4.callable_region.blocks[0].stmts.at(0)
    assert isinstance(stmt, py.Len)


test()
