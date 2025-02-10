from typing import Any, TypeVar, Iterable

from kirin import ir
from kirin.lowering import wraps

from . import stmts
from .runtime import IList

T_Elem = TypeVar("T_Elem")
T_Out = TypeVar("T_Out")
T_Result = TypeVar("T_Result")


@wraps(stmts.Map)
def map(fn: ir.Method[[T_Elem], T_Out], collection: Iterable) -> IList | list: ...


@wraps(stmts.Foldr)
def foldr(
    fn: ir.Method[[T_Elem, T_Out], T_Out], collection: Iterable, init: T_Out
) -> T_Out: ...


@wraps(stmts.Foldl)
def foldl(
    fn: ir.Method[[T_Out, T_Elem], T_Out], collection: Iterable, init: T_Out
) -> T_Out: ...


@wraps(stmts.Scan)
def scan(
    fn: ir.Method[[T_Out, T_Elem], tuple[T_Out, T_Result]],
    collection: Iterable,
    init: T_Out,
) -> tuple[T_Out, IList[T_Result, Any]]: ...


@wraps(stmts.ForEach)
def for_each(fn: ir.Method[[T_Elem], None], collection: Iterable) -> None: ...
