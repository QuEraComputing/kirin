from kirin.decl import info, statement
from kirin.dialects.py import types
from kirin.dialects.py.stmts.dialect import dialect
from kirin.ir import Pure, ResultValue, SSAValue, Statement


@statement(dialect=dialect)
class NewList(Statement):
    name = "list"
    traits = frozenset({Pure()})

    def __init__(self, type: types.PyType, values: tuple[SSAValue, ...]) -> None:
        super().__init__(
            args=values,
            result_types=[
                types.List[type],
            ],
        )


@statement(dialect=dialect)
class Append(Statement):
    name = "append"
    traits = frozenset({})
    lst: SSAValue = info.argument(types.List)
    value: SSAValue = info.argument(types.Any)


@statement(dialect=dialect)
class Len(Statement):
    name = "len"
    traits = frozenset({Pure()})
    value: SSAValue = info.argument(types.Any)
    result: ResultValue = info.result(types.Int)
