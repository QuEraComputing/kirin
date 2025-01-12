import ast
from abc import abstractmethod
from typing import Generic, TypeVar
from dataclasses import dataclass

from kirin import ir, interp, lowering, exceptions
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
    result: ir.ResultValue = info.result(ir.types.Any)


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
        frame: interp.Frame[ir.types.TypeAttribute],
        stmt: GetItem,
    ):
        obj = frame.get(stmt.obj)
        if interp.is_const(obj):  # unwrap const
            obj = obj.type
        index: ir.types.TypeAttribute = frame.get(stmt.index)
        # TODO: replace this when we can multiple dispatch
        if obj.is_subseteq(ir.types.Tuple):
            return self.getitem_tuple(interp, stmt, obj, index)
        elif isinstance(obj, ir.types.PyClass):
            return (ir.types.Any,)
        elif isinstance(obj, ir.types.Generic) and obj.is_subseteq(
            ir.types.List
        ):  # TODO: add type guard
            if index.is_subseteq(ir.types.Int):
                return (obj.vars[0],)
            elif index.is_subseteq(ir.types.Slice):
                return (obj,)
            else:
                return (ir.types.Bottom,)
        else:
            return (ir.types.Any,)

    def getitem_tuple(
        self,
        interp,
        stmt: GetItem,
        obj: ir.types.TypeAttribute,
        index: ir.types.TypeAttribute,
    ):
        if isinstance(obj, ir.types.Generic):
            if index.is_subseteq(ir.types.Int):
                return self.getitem_tuple_index(interp, stmt, obj, index)
            elif index.is_subseteq(ir.types.Slice):
                return self.getitem_tuple_slice(interp, stmt, obj, index)
            else:
                return (ir.types.Bottom,)
        elif isinstance(obj, ir.types.PyClass):
            return (ir.types.Any,)
        else:
            return (ir.types.Bottom,)

    def getitem_tuple_index(
        self,
        interp: TypeInference,
        stmt: GetItem,
        obj: ir.types.Generic,
        index: ir.types.TypeAttribute,
    ):
        if interp.is_const(index) and index.type.is_subseteq(ir.types.Int):
            index_: int = index.data.data
            if obj.vararg and index_ >= len(obj.vars):
                return (obj.vararg.typ,)
            elif index_ < len(obj.vars):
                return (obj.vars[index_],)
            else:
                return (ir.types.Bottom,)
        else:
            return (self.getitem_tuple_union(obj),)

    def getitem_tuple_slice(
        self,
        interp: TypeInference,
        stmt: GetItem,
        obj: ir.types.Generic,
        index: ir.types.TypeAttribute,
    ):
        if interp.is_const(index):
            data: slice = index.data.data
            if obj.vararg and data.stop >= len(obj.vars):
                return (
                    ir.types.Union(
                        *obj.vars[slice(data.start, len(obj.vars), data.step)],
                        obj.vararg.typ,
                    ),
                )
            elif data.stop is None or data.stop < len(obj.vars):
                return (
                    ir.types.Tuple.where(
                        obj.vars[slice(data.start, data.stop, data.step)]
                    ),
                )
            else:  # out of bounds
                return (ir.types.Bottom,)
        else:
            return (ir.types.Tuple[ir.types.Vararg(self.getitem_tuple_union(obj))],)

    def getitem_tuple_union(self, obj: ir.types.Generic):
        if obj.vararg:
            return ir.types.Union(*obj.vars, obj.vararg.typ)
        else:
            return ir.types.Union(*obj.vars)


@dialect.register(key="constprop")
class ConstProp(interp.MethodTable):

    @interp.impl(GetItem)
    def getitem(
        self,
        _: const.Propagate,
        frame: interp.Frame[const.JointResult],
        stmt: GetItem,
    ) -> interp.StatementResult[const.JointResult]:
        obj = frame.get(stmt.obj).const
        index = frame.get(stmt.index).const
        if not isinstance(index, const.Value):
            return (const.JointResult(const.Unknown(), const.Pure()),)

        if isinstance(obj, const.PartialTuple):
            obj = obj.data
            if isinstance(index.data, int) and 0 <= index.data < len(obj):
                return (const.JointResult(obj[index.data], const.Pure()),)
            elif isinstance(index.data, slice):
                start, stop, step = index.data.indices(len(obj))
                return (
                    const.JointResult(
                        const.PartialTuple(obj[start:stop:step]), const.Pure()
                    ),
                )
        return (const.JointResult(const.Unknown(), const.Pure()),)


GetItemLikeStmt = TypeVar("GetItemLikeStmt", bound=ir.Statement)


@dataclass(init=False)
class RewriteGetItem(RewriteRule, Generic[GetItemLikeStmt]):
    target_stmt_type: type[GetItemLikeStmt]
    obj_type: ir.types.TypeAttribute
    getitem_like: GetItemLike[GetItemLikeStmt]

    def __init__(
        self, stmt_type: type[GetItemLikeStmt], obj_type: ir.types.TypeAttribute
    ):
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
