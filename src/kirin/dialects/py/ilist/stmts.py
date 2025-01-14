from typing import Sequence

from kirin import ir
from kirin.decl import info, statement

from ._dialect import dialect

ElemT = ir.types.TypeVar("ElemT")
ListLen = ir.types.TypeVar("ListLen")


@statement(dialect=dialect, init=False)
class New(ir.Statement):
    traits = frozenset({ir.Pure(), ir.FromPythonCall()})
    values: tuple[ir.SSAValue, ...] = info.argument(ElemT)
    result: ir.ResultValue = info.result(ir.types.IList[ElemT])

    def __init__(
        self,
        values: Sequence[ir.SSAValue],
    ) -> None:
        # get elem type
        elem_type = values[0].type
        for v in values:
            elem_type = elem_type.join(v.type)

        result_type = ir.types.IList[elem_type, ir.types.Literal(len(values))]
        super().__init__(
            args=values,
            result_types=(result_type,),
            args_slice={"values": slice(0, len(values))},
        )


OutElemT = ir.types.TypeVar("OutElemT")


@statement(dialect=dialect)
class Map(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})
    fn: ir.SSAValue = info.argument(ir.types.Generic(ir.Method, [ElemT], OutElemT))
    collection: ir.SSAValue = info.argument(ir.types.IList[ElemT, ListLen])
    result: ir.ResultValue = info.result(ir.types.IList[OutElemT, ListLen])


@statement(dialect=dialect)
class FoldR(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})
    fn: ir.SSAValue = info.argument(
        ir.types.Generic(ir.Method, [ElemT, OutElemT], OutElemT)
    )
    collection: ir.SSAValue = info.argument(ir.types.IList[ElemT])
    init: ir.SSAValue = info.argument(OutElemT)
    result: ir.ResultValue = info.result(OutElemT)


@statement(dialect=dialect)
class FoldL(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})
    fn: ir.SSAValue = info.argument(
        ir.types.Generic(ir.Method, [OutElemT, ElemT], OutElemT)
    )
    collection: ir.SSAValue = info.argument(ir.types.IList[ElemT])
    init: ir.SSAValue = info.argument(OutElemT)
    result: ir.ResultValue = info.result(OutElemT)


CarryT = ir.types.TypeVar("CarryT")
ResultT = ir.types.TypeVar("ResultT")


@statement(dialect=dialect)
class Scan(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})
    fn: ir.SSAValue = info.argument(
        ir.types.Generic(
            ir.Method, [OutElemT, ElemT], ir.types.Tuple[OutElemT, ResultT]
        )
    )
    collection: ir.SSAValue = info.argument(ir.types.IList[ElemT, ListLen])
    init: ir.SSAValue = info.argument(OutElemT)
    result: ir.ResultValue = info.result(
        ir.types.Tuple[OutElemT, ir.types.IList[ResultT, ListLen]]
    )


@statement(dialect=dialect)
class ForEach(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})
    fn: ir.SSAValue = info.argument(
        ir.types.Generic(ir.Method, [ElemT], ir.types.NoneType)
    )
    collection: ir.SSAValue = info.argument(ir.types.IList[ElemT])
