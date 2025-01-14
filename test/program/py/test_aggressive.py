# type: ignore

from kirin import ir, types
from kirin.decl import info, statement
from kirin.prelude import basic, basic_no_opt
from kirin.dialects.py import cmp

dialect = ir.Dialect("dummy2")


@statement(dialect=dialect)
class DummyStmt2(ir.Statement):
    name = "dummy2"
    traits = frozenset({ir.FromPythonCall()})
    value: ir.SSAValue = info.argument(types.Int)
    option: ir.PyAttr[str] = info.attribute()
    result: ir.ResultValue = info.result(types.Int)


@basic_no_opt.add(dialect)
def unfolable(x: int, y: int):
    def inner():
        DummyStmt2(x, option=ir.PyAttr("hello"))
        DummyStmt2(y, option=ir.PyAttr("hello"))

    return inner


@basic.add(dialect)(fold=True, aggressive=True)
def main():
    x = DummyStmt2(1, option=ir.PyAttr("hello"))
    x = unfolable(x, x)
    return x()


def test_aggressive_pass():
    const_count = 0
    dummy_count = 0
    for stmt in main.callable_region.walk():
        if isinstance(stmt, DummyStmt2):
            dummy_count += 1
        elif stmt.has_trait(ir.ConstantLike):
            const_count += 1
    assert dummy_count == 3
    assert const_count == 2


@basic(fold=True, aggressive=True)
def should_fold():
    return 1 < 2


def test_should_fold():
    for stmt in should_fold.callable_region.walk():
        assert not isinstance(stmt, cmp.Lt)
