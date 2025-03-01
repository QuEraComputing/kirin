"""The indexing dialect for Python.

This module contains the dialect for the Python indexing syntax, including:

- The `GetItem` statement class.
- A base class `Subscript` for indexing statements.
- A trait `GetItemLike` for indexing statements.
- The lowering pass for the indexing statement.
- The concrete implementation of the indexing statement.
- The constant propagation implementation (special case) of the indexing statement.
- The type inference implementation of the indexing statement.
- A canonical rewrite rule for the rewriting of a given getitem-like
    statement to another getitem-like statement.
"""

import ast
from abc import abstractmethod
from typing import Generic, TypeVar
from dataclasses import dataclass

from kirin import ir, types, interp, lowering, exceptions
from kirin.decl import info, statement
from kirin.analysis import const
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.analysis.typeinfer import TypeInference

dialect = ir.Dialect("py.indexing")

GetItemLikeStmt = TypeVar("GetItemLikeStmt", bound=ir.Statement)


@dataclass(frozen=True, eq=False)
class GetItemLike(ir.StmtTrait, Generic[GetItemLikeStmt]):

    @abstractmethod
    def get_object(self, stmt: GetItemLikeStmt) -> ir.SSAValue: ...

    @abstractmethod
    def get_index(self, stmt: GetItemLikeStmt) -> ir.SSAValue: ...

    @abstractmethod
    def new(
        self, stmt_type: type[GetItemLikeStmt], obj: ir.SSAValue, index: ir.SSAValue
    ) -> GetItemLikeStmt: ...


PyGetItemLikeStmt = TypeVar("PyGetItemLikeStmt", bound="GetItem")


@dataclass(frozen=True, eq=False)
class PyGetItemLike(GetItemLike[PyGetItemLikeStmt]):

    def get_object(self, stmt: PyGetItemLikeStmt) -> ir.SSAValue:
        return stmt.obj

    def get_index(self, stmt: PyGetItemLikeStmt) -> ir.SSAValue:
        return stmt.index

    def new(
        self, stmt_type: type[PyGetItemLikeStmt], obj: ir.SSAValue, index: ir.SSAValue
    ) -> PyGetItemLikeStmt:
        return stmt_type(obj=obj, index=index)


# NOTE: in IR setindex is very different from getindex
# taking Julia's semantics as reference here
@statement
class Subscript(ir.Statement):
    pass


@statement(dialect=dialect)
class GetItem(Subscript):
    name = "getitem"
    traits = frozenset({ir.Pure(), PyGetItemLike(), ir.FromPythonCall()})
    obj: ir.SSAValue = info.argument(print=False)
    index: ir.SSAValue = info.argument(print=False)
    result: ir.ResultValue = info.result(types.Any)


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_Subscript(
        self, state: lowering.LoweringState, node: ast.Subscript
    ) -> lowering.Result:
        value = state.visit(node.value).expect_one()
        slice = state.visit(node.slice).expect_one()
        if isinstance(node.ctx, ast.Load):
            stmt = GetItem(obj=value, index=slice)
        else:
            raise exceptions.DialectLoweringError(
                f"unsupported subscript context {node.ctx}"
            )
        state.append_stmt(stmt)
        return lowering.Result(stmt)


@dialect.register
class Concrete(interp.MethodTable):

    @interp.impl(GetItem)
    def getindex(self, interp, frame: interp.Frame, stmt: GetItem):
        return (frame.get(stmt.obj)[frame.get(stmt.index)],)


@dialect.register(key="typeinfer")
class TypeInfer(interp.MethodTable):

    @interp.impl(GetItem)
    def getitem(
        self,
        interp: TypeInference,
        frame: interp.Frame[types.TypeAttribute],
        stmt: GetItem,
    ):
        obj = frame.get(stmt.obj)
        index: types.TypeAttribute = frame.get(stmt.index)
        # TODO: replace this when we can multiple dispatch
        if obj.is_subseteq(types.Tuple):
            return self.getitem_tuple(interp, stmt, obj, index)
        elif obj.is_subseteq(types.String):
            return (types.String,)
        else:
            return (types.Any,)

    def getitem_tuple(
        self,
        interp,
        stmt: GetItem,
        obj: types.TypeAttribute,
        index: types.TypeAttribute,
    ):
        if isinstance(obj, types.Generic):
            if index.is_subseteq(types.Int):
                return self.getitem_tuple_index(interp, stmt, obj, index)
            elif index.is_subseteq(types.Slice):
                return self.getitem_tuple_slice(interp, stmt, obj, index)
            else:
                return (types.Bottom,)
        elif isinstance(obj, types.PyClass):
            return (types.Any,)
        else:
            return (types.Bottom,)

    def getitem_tuple_index(
        self,
        interp: TypeInference,
        stmt: GetItem,
        obj: types.Generic,
        index: types.TypeAttribute,
    ):
        if index_ := interp.maybe_const(stmt.index, int):
            if obj.vararg and (index_ >= len(obj.vars) or -len(obj.vars) <= index_ < 0):
                return (obj.vararg.typ,)
            elif obj.vars and (
                0 <= index_ < len(obj.vars) or -len(obj.vars) <= index_ < 0
            ):
                return (obj.vars[index_],)
            else:
                return (types.Bottom,)
        else:
            return (self.getitem_tuple_union(obj),)

    def getitem_tuple_slice(
        self,
        interp: TypeInference,
        stmt: GetItem,
        obj: types.Generic,
        index: types.TypeAttribute,
    ):
        if index_ := interp.maybe_const(stmt.index, slice):
            if obj.vararg and index_.stop >= len(obj.vars):
                return (
                    types.Union(
                        *obj.vars[slice(index_.start, len(obj.vars), index_.step)],
                        obj.vararg.typ,
                    ),
                )
            elif index_.stop is None or index_.stop < len(obj.vars):
                return (
                    types.Tuple.where(
                        obj.vars[slice(index_.start, index_.stop, index_.step)]
                    ),
                )
            else:  # out of bounds
                return (types.Bottom,)
        else:
            return (types.Tuple[types.Vararg(self.getitem_tuple_union(obj))],)

    def getitem_tuple_union(self, obj: types.Generic):
        if obj.vararg:
            return types.Union(*obj.vars, obj.vararg.typ)
        else:
            return types.Union(*obj.vars)


@dialect.register(key="constprop")
class ConstProp(interp.MethodTable):

    @interp.impl(GetItem)
    def getitem(
        self,
        _: const.Propagate,
        frame: const.Frame,
        stmt: GetItem,
    ) -> interp.StatementResult[const.Result]:
        obj = frame.get(stmt.obj)
        index = frame.get(stmt.index)
        if not isinstance(index, const.Value):
            return (const.Unknown(),)

        if isinstance(obj, const.Value):
            return (const.Value(obj.data[index.data]),)
        elif isinstance(obj, const.PartialTuple):
            obj = obj.data
            if isinstance(index.data, int) and 0 <= index.data < len(obj):
                return (obj[index.data],)
            elif isinstance(index.data, slice):
                start, stop, step = index.data.indices(len(obj))
                return (const.PartialTuple(obj[start:stop:step]),)
        return (const.Unknown(),)


GetItemLikeStmt = TypeVar("GetItemLikeStmt", bound=ir.Statement)


@dataclass(init=False)
class RewriteGetItem(RewriteRule, Generic[GetItemLikeStmt]):
    target_stmt_type: type[GetItemLikeStmt]
    obj_type: types.TypeAttribute
    getitem_like: GetItemLike[GetItemLikeStmt]

    def __init__(self, stmt_type: type[GetItemLikeStmt], obj_type: types.TypeAttribute):
        trait = stmt_type.get_trait(GetItemLike)
        if trait is None:
            raise ValueError(f"{stmt_type} does not have GetItemLike trait")

        self.obj_type = obj_type
        self.target_stmt_type = stmt_type
        self.getitem_like = trait

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, GetItem):
            return RewriteResult()

        if not node.obj.type.is_subseteq(self.obj_type):
            return RewriteResult()

        node.replace_by(
            self.getitem_like.new(self.target_stmt_type, node.obj, node.index)
        )
        return RewriteResult(has_done_something=True)
