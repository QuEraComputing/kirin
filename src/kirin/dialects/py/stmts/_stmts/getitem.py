from abc import abstractmethod
from dataclasses import dataclass
from typing import Generic, TypeVar

from kirin.decl import info, statement
from kirin.dialects.py import types
from kirin.dialects.py.stmts.dialect import dialect
from kirin.ir import Pure, ResultValue, SSAValue, Statement, StmtTrait

GetItemLikeStmt = TypeVar("GetItemLikeStmt", bound=Statement)


@dataclass(frozen=True, eq=False)
class GetItemLike(StmtTrait, Generic[GetItemLikeStmt]):

    @abstractmethod
    def get_object(self, stmt: GetItemLikeStmt) -> SSAValue: ...

    @abstractmethod
    def get_index(self, stmt: GetItemLikeStmt) -> SSAValue: ...

    @abstractmethod
    def new(
        self, stmt_type: type[GetItemLikeStmt], obj: SSAValue, index: SSAValue
    ) -> GetItemLikeStmt: ...


PyGetItemLikeStmt = TypeVar("PyGetItemLikeStmt", bound="GetItem")


@dataclass(frozen=True, eq=False)
class PyGetItemLike(GetItemLike[PyGetItemLikeStmt]):

    def get_object(self, stmt: PyGetItemLikeStmt) -> SSAValue:
        return stmt.obj

    def get_index(self, stmt: PyGetItemLikeStmt) -> SSAValue:
        return stmt.index

    def new(
        self, stmt_type: type[PyGetItemLikeStmt], obj: SSAValue, index: SSAValue
    ) -> PyGetItemLikeStmt:
        return stmt_type(obj=obj, index=index)


# NOTE: in IR setindex is very different from getindex
# taking Julia's semantics as reference here
@statement
class Subscript(Statement):
    pass


@statement(dialect=dialect)
class GetItem(Subscript):
    name = "getitem"
    traits = frozenset({Pure(), PyGetItemLike()})
    obj: SSAValue = info.argument(print=False)
    index: SSAValue = info.argument(print=False)
    result: ResultValue = info.result(types.Any)


@statement(dialect=dialect)
class SetItem(Subscript):
    name = "setitem"
    obj: SSAValue = info.argument(print=False)
    value: SSAValue = info.argument(print=False)
    index: SSAValue = info.argument(print=False)
