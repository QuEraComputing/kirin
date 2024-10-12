import pytest

from kirin import ir
from kirin.decl import info, statement

dialect = ir.Dialect("my_dialect")


def test_reserved_verify():
    with pytest.raises(ValueError):

        @statement(dialect=dialect)
        class ReserveKeyword(ir.Statement):
            name = "my_statement"
            traits = frozenset({})
            args: ir.SSAValue = info.argument()

    with pytest.raises(ValueError):

        @statement(dialect=dialect)
        class NoAnnotation(ir.Statement):
            name = "my_statement"
            traits = frozenset({})
            no_annotation = info.argument()  # type: ignore

    with pytest.raises(ValueError):

        @statement(dialect=dialect)
        class WrongAnnotation(ir.Statement):
            name = "my_statement"
            traits = frozenset({})
            field: str = info.argument()

    with pytest.raises(ValueError):

        @statement(dialect=dialect)
        class WrongFiledSpecifier(ir.Statement):
            name = "my_statement"
            traits = frozenset({})
            result: ir.ResultValue = info.argument()

    with pytest.raises(ValueError):

        @statement(dialect=dialect)
        class WrongResultAnnotation(ir.Statement):
            name = "my_statement"
            traits = frozenset({})
            result: ir.SSAValue = info.result()
