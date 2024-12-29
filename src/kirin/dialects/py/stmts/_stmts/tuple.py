from kirin.ir import Pure, SSAValue, Statement, ResultValue, types
from kirin.decl import info, statement
from kirin.dialects.py.stmts.dialect import dialect


@statement(dialect=dialect)
class NewTuple(Statement):
    traits = frozenset({Pure()})
    result: ResultValue = info.result()

    def __init__(self, values: tuple[SSAValue, ...]) -> None:
        result_type = types.Generic(tuple, *tuple(value.type for value in values))
        super().__init__(
            args=values,
            result_types=[
                result_type,
            ],
        )
