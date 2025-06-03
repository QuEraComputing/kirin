from kirin import ir, types as ktypes
from kirin.prelude import structural
from kirin.analysis import TypeInference


# stmt_at and results_at taken from kirin type inference tests with
# minimal modification
def stmt_at(kernel: ir.Method, block_id: int, stmt_id: int) -> ir.Statement:
    return kernel.code.body.blocks[block_id].stmts.at(stmt_id)  # type: ignore


def results_at(kernel: ir.Method, block_id: int, stmt_id: int):
    return stmt_at(kernel, block_id, stmt_id).results


def test_tuple_type_infer():

    @structural(typeinfer=True)
    def test(x: bool):
        a = [True, False, True]
        return (a[0], x)

    typeinfer = TypeInference(structural)
    frame, _ = typeinfer.run_analysis(test)

    assert [frame.entries[result] for result in results_at(test, 0, 1)] == [
        ktypes.Generic(tuple, ktypes.Bool, ktypes.Bool)
    ]
