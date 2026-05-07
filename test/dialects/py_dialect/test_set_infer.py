from kirin import ir, types as ktypes
from kirin.prelude import structural
from kirin.analysis import TypeInference
from kirin.dialects import py


def set_stmt_result(kernel: ir.Method):
    stmt = next(
        stmt for stmt in kernel.code.body.blocks[0].stmts if isinstance(stmt, py.set.New)  # type: ignore
    )
    return stmt.results[0]


def test_set_type_infer_homogeneous():

    @structural(typeinfer=True, fold=False)
    def test():
        return {1, 2}

    typeinfer = TypeInference(structural)
    frame, _ = typeinfer.run(test)

    assert frame.entries[set_stmt_result(test)] == ktypes.Set[ktypes.Int]


def test_set_type_infer_empty():

    @structural(typeinfer=True, fold=False)
    def test():
        return set()

    typeinfer = TypeInference(structural)
    frame, _ = typeinfer.run(test)

    assert frame.entries[set_stmt_result(test)] == ktypes.Set[ktypes.Any]
