from kirin.decl import info, statement
from kirin.dialects.py import types
from kirin.dialects.py.stmts.dialect import dialect
from kirin.ir import Pure, ResultValue, SSAValue, Statement


@statement(dialect=dialect, init=False)
class Slice(Statement):
    name = "slice"
    traits = frozenset({Pure()})
    start: SSAValue = info.argument(types.Any)
    stop: SSAValue = info.argument(types.Any)
    step: SSAValue = info.argument(types.Any)
    result: ResultValue = info.result(types.Slice)

    def __init__(self, start: SSAValue, stop: SSAValue, step: SSAValue) -> None:
        if start.type is types.NoneType:
            if stop.type is types.NoneType:
                T = types.Bottom
            else:
                T = stop.type
        else:
            T = start.type

        super().__init__(
            args=(start, stop, step),
            result_types=[types.Slice[types.widen_const(T)]],
            args_slice={"start": 0, "stop": 1, "step": 2},
        )
