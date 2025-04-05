import pytest

from kirin import ir, lowering
from kirin.decl import info, statement
from kirin.prelude import basic_no_opt
from kirin.dialects import cf, py

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

    with pytest.raises(lowering.BuildError):

        @dummy_dialect
        def multi_assign_error():
            (x, y, z) = MultiResult()  # type: ignore
            return x, y, z


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


def test_aug_assign():
    @dummy_dialect
    def aug_assign(y):
        y += 1
        return y

    y = aug_assign.callable_region.blocks[0].args[1]
    const = aug_assign.callable_region.blocks[0].stmts.at(0)
    assert isinstance(const, py.Constant)
    assert const.value.unwrap() == 1
    add = aug_assign.callable_region.blocks[0].stmts.at(1)
    assert isinstance(add, py.binop.Add)
    assert add.lhs is y
    assert add.rhs is const.result


def test_named_expr():

    @dummy_dialect
    def named_expr(y):
        if y := y + 1:
            return y
        return y

    stmt = named_expr.callable_region.blocks[0].stmts.at(1)
    y = named_expr.callable_region.blocks[0].args[1]
    assert isinstance(stmt, py.binop.Add)
    assert stmt.lhs is y
    br = named_expr.callable_region.blocks[0].stmts.at(2)
    assert isinstance(br, cf.ConditionalBranch)
    assert stmt.result is br.cond
