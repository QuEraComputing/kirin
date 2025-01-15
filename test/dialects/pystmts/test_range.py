from kirin.ir import types
from kirin.prelude import basic
from kirin.dialects.py.range import Range


@basic
def new_range(a: int, b: int, c: int):
    x = range(a)
    y = range(a, b)
    z = range(a, b, c)
    return x, y, z


def test_new_range():
    stmt: Range = new_range.code.body.blocks[0].stmts.at(2)
    assert isinstance(stmt.start.type, types.Hinted)
    assert stmt.start.type.data.data == 0
    assert stmt.stop.type.is_subseteq(types.Int)
    assert isinstance(stmt.step.type, types.Hinted)
    assert stmt.step.type.data.data == 1

    stmt: Range = new_range.code.body.blocks[0].stmts.at(4)
    assert stmt.start.type.is_subseteq(types.Int)
    assert stmt.stop.type.is_subseteq(types.Int)
    assert isinstance(stmt.step.type, types.Hinted)
    assert stmt.step.type.data.data == 1

    stmt: Range = new_range.code.body.blocks[0].stmts.at(5)
    assert stmt.start.type.is_subseteq(types.Int)
    assert stmt.stop.type.is_subseteq(types.Int)
    assert stmt.step.type.is_subseteq(types.Int)
