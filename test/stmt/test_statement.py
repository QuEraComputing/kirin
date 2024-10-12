import pytest

from kirin import ir
from kirin.decl import info, statement

dialect = ir.Dialect("my_dialect")


def test_reserved_verify():
    with pytest.raises(ValueError):

        @statement(dialect=dialect)
        class MyStatement(ir.Statement):
            name = "my_statement"
            traits = frozenset({})
            args: ir.SSAValue = info.argument()
