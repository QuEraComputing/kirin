from typing import Sequence

from kirin import ir
from kirin.decl import info, statement

from .runtime import IList
from ._dialect import dialect

ElemT = ir.types.TypeVar("ElemT")
ListLen = ir.types.TypeVar("ListLen")
IListType = ir.types.Generic(IList, ElemT, ListLen)


@statement(dialect=dialect, init=False)
class New(ir.Statement):
    traits = frozenset({ir.Pure(), ir.FromPythonCall()})
    values: tuple[ir.SSAValue, ...] = info.argument(ElemT)
    result: ir.ResultValue = info.result(IListType[ElemT])

    def __init__(
        self,
        values: Sequence[ir.SSAValue],
    ) -> None:
        # get elem type
        if not values:
            elem_type = ir.types.Any
        else:
            elem_type = values[0].type
            for v in values:
                elem_type = elem_type.join(v.type)

        result_type = IListType[elem_type, ir.types.Literal(len(values))]
        super().__init__(
            args=values,
            result_types=(result_type,),
            args_slice={"values": slice(0, len(values))},
        )


@statement(dialect=dialect)
class Push(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})
    lst: ir.SSAValue = info.argument(IListType[ElemT])
    value: ir.SSAValue = info.argument(IListType[ElemT])
    result: ir.ResultValue = info.result(IListType[ElemT])


OutElemT = ir.types.TypeVar("OutElemT")


@statement(dialect=dialect)
class Map(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})
    fn: ir.SSAValue = info.argument(ir.types.Generic(ir.Method, [ElemT], OutElemT))
    collection: ir.SSAValue = info.argument(IListType[ElemT, ListLen])
    result: ir.ResultValue = info.result(IListType[OutElemT, ListLen])


@statement(dialect=dialect)
class Foldr(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})
    fn: ir.SSAValue = info.argument(
        ir.types.Generic(ir.Method, [ElemT, OutElemT], OutElemT)
    )
    collection: ir.SSAValue = info.argument(IListType[ElemT])
    init: ir.SSAValue = info.argument(OutElemT)
    result: ir.ResultValue = info.result(OutElemT)


@statement(dialect=dialect)
class Foldl(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})
    fn: ir.SSAValue = info.argument(
        ir.types.Generic(ir.Method, [OutElemT, ElemT], OutElemT)
    )
    collection: ir.SSAValue = info.argument(IListType[ElemT])
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
    collection: ir.SSAValue = info.argument(IListType[ElemT, ListLen])
    init: ir.SSAValue = info.argument(OutElemT)
    result: ir.ResultValue = info.result(
        ir.types.Tuple[OutElemT, IListType[ResultT, ListLen]]
    )


@statement(dialect=dialect)
class ForEach(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})
    fn: ir.SSAValue = info.argument(
        ir.types.Generic(ir.Method, [ElemT], ir.types.NoneType)
    )
    collection: ir.SSAValue = info.argument(IListType[ElemT])
