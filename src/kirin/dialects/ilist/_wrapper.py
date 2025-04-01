import typing

from kirin import lowering2

from . import stmts
from .runtime import IList

ElemT = typing.TypeVar("ElemT")
OutElemT = typing.TypeVar("OutElemT")
LenT = typing.TypeVar("LenT")
ResultT = typing.TypeVar("ResultT")

# NOTE: we use Callable here to make nested function work.


@typing.overload
def range(stop: int) -> IList[int, typing.Any]: ...


@typing.overload
def range(start: int, stop: int) -> IList[int, typing.Any]: ...


@typing.overload
def range(start: int, stop: int, step: int) -> IList[int, typing.Any]: ...


@lowering2.wraps(stmts.Range)
def range(start: int, stop: int, step: int) -> IList[int, typing.Any]: ...


@lowering2.wraps(stmts.Map)
def map(
    fn: typing.Callable[[ElemT], OutElemT],
    collection: IList[ElemT, LenT] | list[ElemT],
) -> IList[OutElemT, LenT]: ...


@lowering2.wraps(stmts.Foldr)
def foldr(
    fn: typing.Callable[[ElemT, OutElemT], OutElemT],
    collection: IList[ElemT, LenT] | list[ElemT],
    init: OutElemT,
) -> OutElemT: ...


@lowering2.wraps(stmts.Foldl)
def foldl(
    fn: typing.Callable[[OutElemT, ElemT], OutElemT],
    collection: IList[ElemT, LenT] | list[ElemT],
    init: OutElemT,
) -> OutElemT: ...


@lowering2.wraps(stmts.Scan)
def scan(
    fn: typing.Callable[[OutElemT, ElemT], tuple[OutElemT, ResultT]],
    collection: IList[ElemT, LenT] | list[ElemT],
    init: OutElemT,
) -> tuple[OutElemT, IList[ResultT, LenT]]: ...


@lowering2.wraps(stmts.ForEach)
def for_each(
    fn: typing.Callable[[ElemT], typing.Any],
    collection: IList[ElemT, LenT] | list[ElemT],
) -> None: ...
