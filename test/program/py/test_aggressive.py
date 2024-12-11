# type: ignore

from kirin import ir, types
from kirin.decl import info, statement
from kirin.dialects.py import data
from kirin.prelude import basic, basic_no_opt

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


@basic.add(dialect)
def target():
    x = DummyStmt2(1, option="hello")
    DummyStmt2(x, option="hello")
    DummyStmt2(x, option="hello")
    return


def test_aggressive_pass():
    assert target.callable_region.is_structurally_equal(main.callable_region)
