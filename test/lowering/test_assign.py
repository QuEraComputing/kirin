from kirin import ir, lowering
from kirin.decl import info, statement
from kirin.prelude import basic_no_opt
from kirin.dialects import py

dialect = ir.Dialect("test")


@statement(dialect=dialect)
class MultiResult(ir.Statement):
    traits = frozenset({lowering.FromPythonCall()})
    result_a: ir.ResultValue = info.result()
    result_b: ir.ResultValue = info.result()


dummy_dialect = basic_no_opt.add(dialect)


def test_multi_result():
    @dummy_dialect
    def multi_assign():
        (x, y) = MultiResult()  # type: ignore
        return x, y

    stmt = multi_assign.callable_region.blocks[0].stmts.at(0)
    assert isinstance(stmt, MultiResult)
    assert stmt.result_a.name == "x"
    assert stmt.result_b.name == "y"


def test_chain_assign_setattr():

    @dummy_dialect
    def chain_assign(y):
        x = y.z = 1
        return x, y

    stmt = chain_assign.callable_region.blocks[0].stmts.at(1)
    assert isinstance(stmt, py.assign.SetAttribute)
    assert stmt.obj.name == "y"
    assert stmt.attr == "z"
    assert stmt.value.name == "x"
