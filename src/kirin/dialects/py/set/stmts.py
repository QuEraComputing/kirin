from typing import Sequence

from kirin import ir, types, lowering
from kirin.decl import info, statement

from ._dialect import dialect

T = types.TypeVar("T")


@statement(dialect=dialect, init=False)
class New(ir.Statement):
    name = "set"
    traits = frozenset({ir.Pure(), lowering.FromPythonCall()})
    values: tuple[ir.SSAValue, ...] = info.argument(T)
    result: ir.ResultValue = info.result(types.Set[T])

    def __init__(self, values: Sequence[ir.SSAValue]) -> None:
        elem_type: types.TypeAttribute = types.Any
        if values:
            elem_type = values[0].type
            for value in values[1:]:
                elem_type = elem_type.join(value.type)

        super().__init__(
            args=values,
            result_types=(types.Set[elem_type],),
            args_slice={"values": slice(0, len(values))},
        )
