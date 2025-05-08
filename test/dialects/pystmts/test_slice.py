from kirin import types
from kirin.prelude import basic_no_opt
from kirin.dialects import py, ilist
from kirin.dialects.py.slice import SliceAttribute


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


def test_slice_attr():

    @basic_no_opt
    def test():

        return (slice(0, 20), slice(30), slice(1, 40, 5))

    result = test()
    assert result == (
        SliceAttribute(0, 20, None),
        SliceAttribute(None, 30, None),
        SliceAttribute(1, 40, 5),
    )


def test_slice_attr_hash():
    assert hash(SliceAttribute(0, 20, None)) == hash((SliceAttribute, 0, 20, None))


def test_slice_get_index():
    @basic_no_opt
    def test():
        x = slice(0, 20, None)
        y = range(40)
        return y[x]

    assert test() == ilist.IList(range(0, 20, 1))
