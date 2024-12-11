from kirin.decl import info, statement
from kirin.dialects.py.stmts.dialect import dialect
from kirin.ir import Pure, ResultValue, SSAValue, Statement, types


@statement(dialect=dialect, init=False)
class NewList(Statement):
    name = "list"
    traits = frozenset({Pure()})
    values: tuple[SSAValue, ...] = info.argument(types.Any)
    result: ResultValue = info.result()

    def __init__(self, type: types.TypeAttribute, values: tuple[SSAValue, ...]) -> None:
        super().__init__(
            args=values,
            result_types=[
                types.List[type],
            ],
            args_slice={"values": slice(0, len(values))},
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
