from typing import Sequence

from kirin import ir, types, lowering
from kirin.decl import info, statement

from .runtime import IList
from ._dialect import dialect

ElemT = types.TypeVar("ElemT")
ListLen = types.TypeVar("ListLen")
IListType = types.Generic(IList, ElemT, ListLen)


@statement(dialect=dialect)
class Range(ir.Statement):
    name = "range"
    traits = frozenset({ir.Pure(), lowering.FromPythonRangeLike()})
    start: ir.SSAValue = info.argument(types.Int)
    stop: ir.SSAValue = info.argument(types.Int)
    step: ir.SSAValue = info.argument(types.Int)
    result: ir.ResultValue = info.result(IListType[types.Int, types.Any])


@statement(dialect=dialect, init=False)
class New(ir.Statement):
    traits = frozenset({ir.Pure(), lowering.FromPythonCall()})
    values: tuple[ir.SSAValue, ...] = info.argument(ElemT)
    elem_type: types.TypeAttribute = info.attribute()
    result: ir.ResultValue = info.result(IListType[ElemT])

    def __init__(
        self,
        values: Sequence[ir.SSAValue],
        elem_type: types.TypeAttribute | None = None,
    ) -> None:
        if not elem_type:
            elem_type = types.Any
        result_type = IListType[elem_type, types.Literal(len(values))]
        super().__init__(
            args=values,
            result_types=(result_type,),
            args_slice={"values": slice(0, len(values))},
            attributes={"elem_type": elem_type},
        )


@statement(dialect=dialect)
class Push(ir.Statement):
    traits = frozenset({ir.Pure(), lowering.FromPythonCall()})
    lst: ir.SSAValue = info.argument(IListType[ElemT])
    value: ir.SSAValue = info.argument(IListType[ElemT])
    result: ir.ResultValue = info.result(IListType[ElemT])


OutElemT = types.TypeVar("OutElemT")


@statement(dialect=dialect)
class Map(ir.Statement):
    traits = frozenset({ir.MaybePure(), lowering.FromPythonCall()})
    purity: bool = info.attribute(default=False)
    fn: ir.SSAValue = info.argument(types.MethodType[[ElemT], OutElemT])
    collection: ir.SSAValue = info.argument(IListType[ElemT, ListLen])
    result: ir.ResultValue = info.result(IListType[OutElemT, ListLen])


@statement(dialect=dialect)
class Foldr(ir.Statement):
    traits = frozenset({ir.MaybePure(), lowering.FromPythonCall()})
    purity: bool = info.attribute(default=False)
    fn: ir.SSAValue = info.argument(
        types.Generic(ir.Method, [ElemT, OutElemT], OutElemT)
    )
    collection: ir.SSAValue = info.argument(IListType[ElemT])
    init: ir.SSAValue = info.argument(OutElemT)
    result: ir.ResultValue = info.result(OutElemT)


@statement(dialect=dialect)
class Foldl(ir.Statement):
    traits = frozenset({ir.MaybePure(), lowering.FromPythonCall()})
    purity: bool = info.attribute(default=False)
    fn: ir.SSAValue = info.argument(
        types.Generic(ir.Method, [OutElemT, ElemT], OutElemT)
    )
    collection: ir.SSAValue = info.argument(IListType[ElemT])
    init: ir.SSAValue = info.argument(OutElemT)
    result: ir.ResultValue = info.result(OutElemT)


CarryT = types.TypeVar("CarryT")
ResultT = types.TypeVar("ResultT")


@statement(dialect=dialect)
class Scan(ir.Statement):
    traits = frozenset({ir.MaybePure(), lowering.FromPythonCall()})
    purity: bool = info.attribute(default=False)
    fn: ir.SSAValue = info.argument(
        types.Generic(ir.Method, [OutElemT, ElemT], types.Tuple[OutElemT, ResultT])
    )
    collection: ir.SSAValue = info.argument(IListType[ElemT, ListLen])
    init: ir.SSAValue = info.argument(OutElemT)
    result: ir.ResultValue = info.result(
        types.Tuple[OutElemT, IListType[ResultT, ListLen]]
    )


@statement(dialect=dialect)
class ForEach(ir.Statement):
    traits = frozenset({ir.MaybePure(), lowering.FromPythonCall()})
    purity: bool = info.attribute(default=False)
    fn: ir.SSAValue = info.argument(types.Generic(ir.Method, [ElemT], types.NoneType))
    collection: ir.SSAValue = info.argument(IListType[ElemT])
