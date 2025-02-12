from kirin import ir, types
from kirin.prelude import basic
from kirin.analysis import const


@basic(typeinfer=True, fold=True)
def unstable(x: int):  # type: ignore
    y = x + 1
    if y > 10:
        z = y
    else:
        z = y + 1.2
    return z


def test_apply_type():
    def stmt_at(block_id, stmt_id: int) -> ir.Statement:
        return unstable.callable_region.blocks[block_id].stmts.at(stmt_id)  # type: ignore

    assert stmt_at(0, 0).results.types == [types.Int]
    assert stmt_at(0, 0).results[0].hints.get("const") == const.Value(1)
    assert stmt_at(0, 1).results.types == [types.Int]
    assert stmt_at(0, 2).results.types == [types.Int]
    assert stmt_at(0, 2).results[0].hints.get("const") == const.Value(10)
    assert stmt_at(0, 3).results.types == [types.Bool]

    assert stmt_at(1, 0).results.types == [types.Int]
    assert stmt_at(2, 0).results.types == [types.Float]
    assert stmt_at(2, 0).results[0].hints.get("const") == const.Value(1.2)
    assert stmt_at(2, 1).results.types == [types.Float]

    stmt = stmt_at(3, 0)
    assert stmt.args[0].type == (types.Int | types.Float)
