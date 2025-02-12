from kirin import ir, types
from kirin.prelude import basic_no_opt
from kirin.analysis.typeinfer import TypeInference


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
    frame, ret = infer.run_analysis(unstable, (types.Int,))
    assert ret == types.Union(types.Int, types.Float)

    def stmt_at(block_id, stmt_id) -> ir.Statement:
        return unstable.code.body.blocks[block_id].stmts.at(stmt_id)  # type: ignore

    def results_at(block_id, stmt_id):
        return stmt_at(block_id, stmt_id).results

    assert [frame.entries[result] for result in results_at(0, 0)] == [types.Int]
    assert [frame.entries[result] for result in results_at(0, 1)] == [types.Int]
    assert [frame.entries[result] for result in results_at(0, 2)] == [types.Int]
    assert [frame.entries[result] for result in results_at(0, 3)] == [types.Bool]

    assert [frame.entries[result] for result in results_at(1, 0)] == [types.Int]
    assert [frame.entries[result] for result in results_at(2, 0)] == [types.Float]
    assert [frame.entries[result] for result in results_at(2, 1)] == [types.Float]

    stmt = stmt_at(3, 0)
    assert frame.entries[stmt.args[0]] == (types.Int | types.Float)
