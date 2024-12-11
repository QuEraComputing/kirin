from kirin import ir
from kirin.analysis.typeinfer import TypeInference
from kirin.ir import types
from kirin.prelude import basic_no_opt


def test_untable_branch():
    @basic_no_opt
    def unstable(x: int):  # type: ignore
        y = x + 1
        if y > 10:
            z = y
        else:
            z = y + 1.2
        return z

    infer = TypeInference(dialects=unstable.dialects)
    results = infer.eval(unstable, (types.Int,)).expect()
    assert results == types.Union(types.Int, types.Float)

    def stmt_at(block_id, stmt_id) -> ir.Statement:
        return unstable.code.body.blocks[block_id].stmts.at(stmt_id)  # type: ignore

    def results_at(block_id, stmt_id):
        return stmt_at(block_id, stmt_id).results

    assert [infer.results[result] for result in results_at(0, 0)] == [
        types.Const(1, types.Int)
    ]
    assert [infer.results[result] for result in results_at(0, 1)] == [types.Int]
    assert [infer.results[result] for result in results_at(0, 2)] == [
        types.Const(10, types.Int)
    ]
    assert [infer.results[result] for result in results_at(0, 3)] == [types.Bool]

    assert [infer.results[result] for result in results_at(1, 0)] == [types.Int]
    assert [infer.results[result] for result in results_at(2, 0)] == [
        types.Const(1.2, types.Float)
    ]
    assert [infer.results[result] for result in results_at(2, 1)] == [types.Float]

    stmt = stmt_at(3, 0)
    assert infer.results[stmt.args[0]] == (types.Int | types.Float)
