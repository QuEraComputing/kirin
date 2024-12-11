import pytest

from kirin import ir, types
from kirin.decl import info, statement
from kirin.dialects.py import data
from kirin.exceptions import DialectLoweringError
from kirin.prelude import basic

dialect = ir.Dialect("dummy")


@statement(dialect=dialect)
class DummyStmt(ir.Statement):
    name = "dummy"
    value: ir.SSAValue = info.argument(types.Int)
    option: data.PyAttr[str] = info.attribute()
    result: ir.ResultValue = info.result(types.Int)


def test_attribute_lowering():
    @basic.add(dialect)
    def test(x: int):
        return DummyStmt(x, option="attr")  # type: ignore

    option = test.code.body.blocks[0].stmts.at(0).option  # type: ignore
    assert isinstance(option, data.PyAttr) and option.data == "attr"

    with pytest.raises(DialectLoweringError):

        @basic.add(dialect)
        def not_working(x: int):
            return DummyStmt(x, option=x)  # type: ignore
