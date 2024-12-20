# type: ignore

from kirin import ir, types
from kirin.decl import info, statement
from kirin.prelude import basic, basic_no_opt
from kirin.dialects.py import data

dialect = ir.Dialect("dummy2")


@statement(dialect=dialect)
class DummyStmt2(ir.Statement):
    name = "dummy2"
    value: ir.SSAValue = info.argument(types.Int)
    option: data.PyAttr[str] = info.attribute()
    result: ir.ResultValue = info.result(types.Int)


@basic_no_opt.add(dialect)
def unfolable(x: int, y: int):
    def inner():
        DummyStmt2(x, option="hello")
        DummyStmt2(y, option="hello")

    return inner


@basic.add(dialect)(fold=True, aggressive=True)
def main():
    x = DummyStmt2(1, option="hello")
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
