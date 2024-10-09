from kirin.dialects.py import stmts, types
from kirin.prelude import basic


@basic
def explicit_slice():
    x = slice(1, 2, 3)
    y = slice(1, 2)
    z = slice(1)
    return x, y, z


def test_explicit_slice():
    stmt: stmts.Slice = explicit_slice.code.body.blocks[0].stmts.at(3)
    assert stmt.start.type.is_subseteq(types.Int)
    assert stmt.stop.type.is_subseteq(types.Int)
    assert stmt.step.type.is_subseteq(types.Int)
    assert stmt.result.type.is_subseteq(types.Slice[types.Int])

    stmt: stmts.Slice = explicit_slice.code.body.blocks[0].stmts.at(7)
    assert stmt.start.type.is_subseteq(types.Int)
    assert stmt.stop.type.is_subseteq(types.Int)
    assert stmt.step.type.is_subseteq(types.NoneType)
    assert stmt.result.type.is_subseteq(types.Slice[types.Int])

    stmt: stmts.Slice = explicit_slice.code.body.blocks[0].stmts.at(11)
    assert stmt.start.type.is_subseteq(types.NoneType)
    assert stmt.stop.type.is_subseteq(types.Int)
    assert stmt.step.type.is_subseteq(types.NoneType)
    assert stmt.result.type.is_subseteq(types.Slice[types.Int])
