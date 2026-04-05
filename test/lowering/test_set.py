from kirin import types, lowering
from kirin.dialects import cf, py, func
from kirin.dialects.lowering import func as func_lowering

lower = lowering.Python(
    [cf, func, py.base, py.constant, py.set, py.assign, func_lowering]
)


def test_set_literal_lowers_to_new():

    def set_literal():
        x = {1, 2}
        return x

    code = lower.python_function(set_literal)

    set_stmt = next(
        stmt for stmt in code.body.blocks[0].stmts if isinstance(stmt, py.set.New)  # type: ignore
    )

    assert isinstance(set_stmt, py.set.New)
    assert len(set_stmt.values) == 2
    assert set_stmt.result.type.is_subseteq(types.Set)


def test_empty_set_call_lowers_to_new():

    def empty_set():
        x = set()
        return x

    code = lower.python_function(empty_set)

    set_stmt = next(
        stmt for stmt in code.body.blocks[0].stmts if isinstance(stmt, py.set.New)  # type: ignore
    )

    assert isinstance(set_stmt, py.set.New)
    assert len(set_stmt.values) == 0
    assert set_stmt.result.type.is_subseteq(types.Set)
