from kirin import ir, lowering
from kirin.decl import info, statement
from kirin.prelude import basic_no_opt
from kirin.rewrite.cse import CommonSubexpressionElimination
from kirin.rewrite.walk import Walk

dialect = ir.Dialect("test")


@statement(dialect=dialect)
class MultiResult(ir.Statement):
    traits = frozenset({ir.Pure(), lowering.FromPythonCall()})
    result_a: ir.ResultValue = info.result()
    result_b: ir.ResultValue = info.result()


dummy_dialect = basic_no_opt.add(dialect)


def test_multi_result():
    @dummy_dialect
    def duplicated():
        x, y = MultiResult()  # type: ignore
        a, b = MultiResult()  # type: ignore
        return x + a, y + b

    stmt_0 = duplicated.callable_region.blocks[0].stmts.at(0)
    stmt_1 = duplicated.callable_region.blocks[0].stmts.at(1)
    assert isinstance(stmt_0, MultiResult)
    assert isinstance(stmt_1, MultiResult)

    Walk(CommonSubexpressionElimination()).rewrite(duplicated.code)

    stmt_0 = duplicated.callable_region.blocks[0].stmts.at(0)
    stmt_1 = duplicated.callable_region.blocks[0].stmts.at(1)
    assert isinstance(stmt_0, MultiResult)
    assert not isinstance(stmt_1, MultiResult)
