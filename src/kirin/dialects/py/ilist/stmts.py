from kirin import ir
from kirin.decl import info, statement

from ._dialect import dialect

ElemT = ir.types.TypeVar("ElemT")


@statement(dialect=dialect)
class New(ir.Statement):
    traits = frozenset({ir.Pure()})
    values: tuple[ir.SSAValue, ...] = info.argument(ElemT)
    result: ir.ResultValue = info.result(ir.types.List[ElemT])


OutElemT = ir.types.TypeVar("OutElemT")


@statement(dialect=dialect)
class Map(ir.Statement):
    fn: ir.SSAValue = info.argument(ir.types.Generic(ir.Method, [ElemT], OutElemT))
    collection: ir.SSAValue = info.argument(ir.types.List[ElemT])
    result: ir.ResultValue = info.result(ir.types.List[OutElemT])


@statement(dialect=dialect)
class FoldR(ir.Statement):
    fn: ir.SSAValue = info.argument(
        ir.types.Generic(ir.Method, [ElemT, OutElemT], OutElemT)
    )
    collection: ir.SSAValue = info.argument(ir.types.List[ElemT])
    init: ir.SSAValue = info.argument(OutElemT)
    result: ir.ResultValue = info.result(OutElemT)


@statement(dialect=dialect)
class FoldL(ir.Statement):
    fn: ir.SSAValue = info.argument(
        ir.types.Generic(ir.Method, [OutElemT, ElemT], OutElemT)
    )
    collection: ir.SSAValue = info.argument(ir.types.List[ElemT])
    init: ir.SSAValue = info.argument(OutElemT)
    result: ir.ResultValue = info.result(OutElemT)


@statement(dialect=dialect)
class Scan(ir.Statement):
    fn: ir.SSAValue = info.argument(
        ir.types.Generic(ir.Method, [ElemT, OutElemT], OutElemT)
    )
    collection: ir.SSAValue = info.argument(ir.types.List[ElemT])
    init: ir.SSAValue = info.argument(OutElemT)
    result: ir.ResultValue = info.result(ir.types.List[OutElemT])


@statement(dialect=dialect)
class ForEach(ir.Statement):
    fn: ir.SSAValue = info.argument(
        ir.types.Generic(ir.Method, [ElemT], ir.types.NoneType)
    )
    collection: ir.SSAValue = info.argument(ir.types.List[ElemT])
