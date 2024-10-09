from kirin.dialects import fcf
from kirin.dialects.py import stmts, types
from kirin.prelude import basic


@basic
def new_range(a: int, b: int, c: int):
    x = range(a)
    y = range(a, b)
    z = range(a, b, c)
    return x, y, z


def test_new_range():
    stmt: stmts.Range = new_range.code.body.blocks[0].stmts.at(2)
    assert isinstance(stmt.start.type, types.PyConst)
    assert stmt.start.type.data == 0
    assert stmt.stop.type.is_subseteq(types.Int)
    assert isinstance(stmt.step.type, types.PyConst)
    assert stmt.step.type.data == 1

    stmt: stmts.Range = new_range.code.body.blocks[0].stmts.at(4)
    assert stmt.start.type.is_subseteq(types.Int)
    assert stmt.stop.type.is_subseteq(types.Int)
    assert isinstance(stmt.step.type, types.PyConst)
    assert stmt.step.type.data == 1

    stmt: stmts.Range = new_range.code.body.blocks[0].stmts.at(5)
    assert stmt.start.type.is_subseteq(types.Int)
    assert stmt.stop.type.is_subseteq(types.Int)
    assert stmt.step.type.is_subseteq(types.Int)


@basic
def add1(x: int):
    return x + 1.0


@basic(typeinfer=True)
def map_range(x: range):
    return fcf.Map(add1, x)


def test_map_range():
    assert map_range.return_type.is_subtype(types.List[types.Float])
    assert map_range(range(10)) == [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]
