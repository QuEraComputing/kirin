from kirin import types
from kirin.prelude import basic_no_opt
from kirin.dialects import py


@basic_no_opt
def explicit_slice():
    x = slice(1, 2, 3)
    y = slice(1, 2)
    z = slice(1)
    return x, y, z


@basic_no_opt
def wrong_slice():
    x = slice(None, None, None)
    y = slice(None, None, 1)
    return x, y


def test_explicit_slice():
    stmt: py.slice.Slice = explicit_slice.code.body.blocks[0].stmts.at(3)
    assert stmt.start.type.is_subseteq(types.Int)
    assert stmt.stop.type.is_subseteq(types.Int)
    assert stmt.step.type.is_subseteq(types.Int)
    assert stmt.result.type.is_subseteq(types.Slice[types.Int])

    stmt: py.slice.Slice = explicit_slice.code.body.blocks[0].stmts.at(7)
    assert stmt.start.type.is_subseteq(types.Int)
    assert stmt.stop.type.is_subseteq(types.Int)
    assert stmt.step.type.is_subseteq(types.NoneType)
    assert stmt.result.type.is_subseteq(types.Slice[types.Int])

    stmt: py.slice.Slice = explicit_slice.code.body.blocks[0].stmts.at(11)
    assert stmt.start.type.is_subseteq(types.NoneType)
    assert stmt.stop.type.is_subseteq(types.Int)
    assert stmt.step.type.is_subseteq(types.NoneType)
    assert stmt.result.type.is_subseteq(types.Slice[types.Int])


def test_wrong_slice():
    stmt: py.slice.Slice = wrong_slice.code.body.blocks[0].stmts.at(3)
    assert stmt.result.type.is_subseteq(types.Bottom)

    stmt: py.slice.Slice = wrong_slice.code.body.blocks[0].stmts.at(7)
    assert stmt.result.type.is_subseteq(types.Bottom)
