from typing import Any, Literal

from kirin import rewrite
from kirin.prelude import basic
from kirin.dialects import py, ilist


def test():
    rule = rewrite.Fixpoint(
        rewrite.Walk(rewrite.Chain(ilist.rewrite.HintLen(), rewrite.ConstantFold()))
    )

    @basic
    def len_func(xs: ilist.IList[int, Literal[3]]):
        return len(xs)

    @basic
    def len_func3(xs: ilist.IList[int, Any]):
        return len(xs)

    rule.rewrite(len_func.code)
    rule.rewrite(len_func3.code)

    stmt = len_func.callable_region.blocks[0].stmts.at(0)
    assert isinstance(stmt, py.Constant)
    assert stmt.value.unwrap() == 3

    stmt = len_func3.callable_region.blocks[0].stmts.at(0)
    assert isinstance(stmt, py.Len)
