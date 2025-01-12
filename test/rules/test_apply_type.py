from kirin import types
from kirin.prelude import basic


@basic(typeinfer=True, fold=False)
def unstable(x: int):  # type: ignore
    y = x + 1
    if y > 10:
        z = y
    else:
        z = y + 1.2
    return z


def test_apply_type():
    unstable.code.print()

    def stmt_at(block_id, stmt_id):
        return unstable.code.body.blocks[block_id].stmts.at(stmt_id)  # type: ignore

    assert stmt_at(0, 0).results.types == [types.Hinted(types.Int, 1)]
    assert stmt_at(0, 1).results.types == [types.Int]
    assert stmt_at(0, 2).results.types == [types.Hinted(types.Int, 10)]
    assert stmt_at(0, 3).results.types == [types.Bool]

    assert stmt_at(1, 0).results.types == [types.Int]
    assert stmt_at(2, 0).results.types == [types.Hinted(types.Float, 1.2)]
    assert stmt_at(2, 1).results.types == [types.Float]

    stmt = stmt_at(3, 0)
    assert stmt.args[0].type == (types.Int | types.Float)
