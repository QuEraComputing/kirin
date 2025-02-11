from typing import TypeVar

from kirin import ir
from kirin.lowering import wraps

from . import stmts
from .runtime import IList

ElemT = TypeVar("ElemT")
OutElemT = TypeVar("OutElemT")
LenT = TypeVar("LenT")
ResultT = TypeVar("ResultT")


@wraps(stmts.Map)
def map(
    fn: ir.Method[[ElemT], OutElemT], collection: IList[ElemT, LenT] | list[ElemT]
) -> IList[OutElemT, LenT]: ...


@wraps(stmts.Foldr)
def foldr(
    fn: ir.Method[[ElemT, OutElemT], OutElemT],
    collection: IList[ElemT, LenT] | list[ElemT],
    init: OutElemT,
) -> OutElemT: ...


@wraps(stmts.Foldl)
def foldl(
    fn: ir.Method[[OutElemT, ElemT], OutElemT],
    collection: IList[ElemT, LenT] | list[ElemT],
    init: OutElemT,
) -> OutElemT: ...


@wraps(stmts.Scan)
def scan(
    fn: ir.Method[[OutElemT, ElemT], tuple[OutElemT, ResultT]],
    collection: IList[ElemT, LenT] | list[ElemT],
    init: OutElemT,
) -> tuple[OutElemT, IList[ResultT, LenT]]: ...


@wraps(stmts.ForEach)
def for_each(
    fn: ir.Method[[ElemT], None], collection: IList[ElemT, LenT] | list[ElemT]
) -> None: ...
