from typing import cast

from kirin import types
from kirin.prelude import basic
from kirin.analysis import const
from kirin.dialects.py.range import Range


@basic
def new_range(a: int, b: int, c: int):
    x = range(a)
    y = range(a, b)
    z = range(a, b, c)
    return x, y, z


new_range.print()


def test_new_range():
    stmt = cast(Range, new_range.callable_region.blocks[0].stmts.at(2))
    assert isinstance(hint := stmt.start.hints.get("const"), const.Value)
    assert hint.data == 0
    assert stmt.stop.type.is_subseteq(types.Int)
    assert isinstance(hint := stmt.step.hints.get("const"), const.Value)
    assert hint.data == 1

    stmt = cast(Range, new_range.callable_region.blocks[0].stmts.at(4))
    assert stmt.start.type.is_subseteq(types.Int)
    assert stmt.stop.type.is_subseteq(types.Int)
    assert stmt.step.type.is_subseteq(types.Int)
    assert isinstance(hint := stmt.step.hints.get("const"), const.Value)
    assert hint.data == 1

    stmt = cast(Range, new_range.callable_region.blocks[0].stmts.at(5))
    assert stmt.start.type.is_subseteq(types.Int)
    assert stmt.stop.type.is_subseteq(types.Int)
    assert stmt.step.type.is_subseteq(types.Int)
