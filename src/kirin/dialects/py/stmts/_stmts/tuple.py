from kirin.decl import info, statement
from kirin.dialects.py import types
from kirin.dialects.py.stmts.dialect import dialect
from kirin.ir import Pure, ResultValue, SSAValue, Statement


@statement(dialect=dialect)
class NewTuple(Statement):
    traits = frozenset({Pure()})
    result: ResultValue = info.result()

    def __init__(self, values: tuple[SSAValue, ...]) -> None:
        result_type = types.PyGeneric(tuple, *tuple(value.type for value in values))  # type: ignore
        super().__init__(
            args=values,
            result_types=[
                result_type,
            ],
        )
