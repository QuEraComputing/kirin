from kirin.ir import types
from kirin.interp import Frame, Result, MethodTable, impl

from . import _stmts as py
from .dialect import dialect


@dialect.register(key="typeinfer")
class TypeInfer(MethodTable):
    # NOTE: const always contains the acutal value, so we can just return the type
    @impl(py.Constant)
    def constant(
        self, interp, frame: Frame, stmt: py.Constant
    ) -> Result[types.TypeAttribute]:
        # NOTE: stmt.result.type should be verified by typecheck
        return (stmt.result.type,)

    @impl(py.Alias)
    def alias(
        self, interp, frame: Frame, stmt: py.Alias
    ) -> Result[types.TypeAttribute]:
        return (frame.get(stmt.value),)  # just forward the type

    @impl(py.NewTuple)
    def new_tuple(
        self, interp, frame: Frame[types.TypeAttribute], stmt: py.NewTuple
    ) -> Result[types.TypeAttribute]:
        return (types.Tuple.where(frame.get_values(stmt.args)),)  # make 3.10 happy

    @impl(py.Add, types.Float, types.Float)
    @impl(py.Add, types.Float, types.Int)
    @impl(py.Add, types.Int, types.Float)
    def addf(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.Float,)

    @impl(py.Add, types.Int, types.Int)
    def addi(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.Int,)

    @impl(py.Add, types.PyClass(list), types.PyClass(list))
    def add_list(self, interp, frame: Frame[types.TypeAttribute], stmt):
        # TODO: solve the type param
        lhs = frame.get(stmt.lhs)
        if isinstance(lhs, types.PyClass):  # add Any as type param
            lhs = types.List
        rhs = frame.get(stmt.rhs)
        if isinstance(rhs, types.PyClass):  # add Any as type param
            rhs = types.List
        return (types.List,)

    @impl(py.Add, types.PyClass(tuple), types.PyClass(tuple))
    def add_tuple(self, interp, frame: Frame[types.TypeAttribute], stmt):
        lhs = frame.get(stmt.lhs)
        rhs = frame.get(stmt.rhs)
        if isinstance(lhs, types.Generic) and isinstance(rhs, types.Generic):
            return (types.Generic(tuple, *(lhs.vars + rhs.vars)),)
        else:
            return (types.PyClass(tuple),)  # no type param, so unknown

    @impl(py.Sub, types.Float, types.Float)
    @impl(py.Sub, types.Float, types.Int)
    @impl(py.Sub, types.Int, types.Float)
    def subf(self, *_) -> Result[types.TypeAttribute]:
        return (types.Float,)

    @impl(py.Sub, types.Int, types.Int)
    def subi(self, *_) -> Result[types.TypeAttribute]:
        return (types.Int,)

    @impl(py.Sub, types.Float, types.Float)
    @impl(py.Sub, types.Float, types.Int)
    @impl(py.Sub, types.Int, types.Float)
    def multf(self, *_) -> Result[types.TypeAttribute]:
        return (types.Float,)

    @impl(py.Mult, types.Int, types.Int)
    def multi(self, *_) -> Result[types.TypeAttribute]:
        return (types.Int,)

    @impl(py.Div)
    def divf(self, *_) -> Result[types.TypeAttribute]:
        return (types.Float,)

    @impl(py.Mod, types.Float, types.Float)
    @impl(py.Mod, types.Float, types.Int)
    @impl(py.Mod, types.Int, types.Float)
    def modf(self, *_) -> Result[types.TypeAttribute]:
        return (types.Float,)

    @impl(py.Mod, types.Int, types.Int)
    def modi(self, *_) -> Result[types.TypeAttribute]:
        return (types.Int,)

    @impl(py.UAdd)
    @impl(py.USub)
    def uadd(
        self, interp, frame: Frame[types.TypeAttribute], stmt: py.UnaryOp
    ) -> Result[types.TypeAttribute]:
        return (frame.get(stmt.value),)

    @impl(py.Eq)
    @impl(py.NotEq)
    @impl(py.Lt)
    @impl(py.LtE)
    @impl(py.Gt)
    @impl(py.GtE)
    def cmp(self, interp, frame, stmt: py.Cmp) -> Result[types.TypeAttribute]:
        return (types.Bool,)

    @impl(py.And)
    @impl(py.Or)
    def boolop(self, interp, frame, stmt: py.BoolOp) -> Result[types.TypeAttribute]:
        return (types.Bool,)

    @impl(py.Not)
    def not_(self, interp, frame, stmt: py.Not) -> Result[types.TypeAttribute]:
        return (types.Bool,)

    @impl(py.BitAnd, types.Int, types.Int)
    def bit_andi(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.Int,)

    @impl(py.BitAnd, types.Bool, types.Bool)
    def bit_andb(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.Bool,)

    @impl(py.BitOr, types.Int, types.Int)
    def bit_ori(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.Int,)

    @impl(py.BitOr, types.Bool, types.Bool)
    def bit_orb(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.Bool,)

    @impl(py.BitXor, types.Int, types.Int)
    def bit_xori(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.Int,)

    @impl(py.BitXor, types.Bool, types.Bool)
    def bit_xorb(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.Bool,)

    @impl(py.Invert, types.Int)
    def invert(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.Int,)

    @impl(py.LShift, types.Int)
    def lshift(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.Int,)

    @impl(py.RShift, types.Int)
    def rshift(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.Int,)

    @impl(py.FloorDiv, types.Float, types.Float)
    @impl(py.FloorDiv, types.Int, types.Float)
    @impl(py.FloorDiv, types.Float, types.Int)
    def floor_divf(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.Float,)

    @impl(py.FloorDiv, types.Int, types.Int)
    def floor_divi(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.Int,)

    @impl(py.Pow, types.Float, types.Float)
    @impl(py.Pow, types.Float, types.Int)
    @impl(py.Pow, types.Int, types.Float)
    def powf(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.Float,)

    @impl(py.Pow, types.Int, types.Int)
    def powi(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.Int,)

    @impl(py.MatMult)
    def mat_mult(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        raise NotImplementedError("np.array @ np.array not implemented")

    @impl(py.GetItem)
    def getitem(
        self,
        interp,
        frame: Frame[types.TypeAttribute],
        stmt: py.GetItem,
    ) -> Result[types.TypeAttribute]:
        obj = frame.get(stmt.obj)
        if isinstance(obj, types.Const):  # unwrap const
            obj = obj.typ
        index: types.TypeAttribute = frame.get(stmt.index)
        # TODO: replace this when we can multiple dispatch
        if obj.is_subseteq(types.Tuple):
            return self.getitem_tuple(interp, stmt, obj, index)
        elif isinstance(obj, types.PyClass):
            return (types.Any,)
        elif isinstance(obj, types.Generic) and obj.is_subseteq(
            types.List
        ):  # TODO: add type guard
            if index.is_subseteq(types.Int):
                return (obj.vars[0],)
            elif index.is_subseteq(types.Slice):
                return (obj,)
            else:
                return (types.Bottom,)
        else:
            return (types.Any,)

    def getitem_tuple(
        self,
        interp,
        stmt: py.GetItem,
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
        interp,
        stmt: py.GetItem,
        obj: types.Generic,
        index: types.TypeAttribute,
    ):
        if isinstance(index, types.Const):  # const
            if obj.vararg and index.data >= len(obj.vars):
                return (obj.vararg.typ,)
            elif index.data < len(obj.vars):
                return (obj.vars[index.data],)
            else:
                return (types.Bottom,)
        else:
            return (self.getitem_tuple_union(obj),)

    def getitem_tuple_slice(
        self,
        interp,
        stmt: py.GetItem,
        obj: types.Generic,
        index: types.TypeAttribute,
    ):
        if isinstance(index, types.Const):
            data: slice = index.data
            if obj.vararg and data.stop >= len(obj.vars):
                return (
                    types.Union(
                        *obj.vars[slice(data.start, len(obj.vars), data.step)],
                        obj.vararg.typ,
                    ),
                )
            elif data.stop is None or data.stop < len(obj.vars):
                return (
                    types.Tuple.where(
                        obj.vars[slice(data.start, data.stop, data.step)]
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

    @impl(py.Abs, types.Int)
    def absi(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.Int,)

    @impl(py.Abs, types.Float)
    def absf(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.Float,)

    @impl(py.SetItem)
    def setindex(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.NoneType,)

    @impl(py.Is)
    def is_(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.Bool,)

    @impl(py.IsNot)
    def is_not(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.Bool,)

    @impl(py.In)
    def in_(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.Bool,)

    @impl(py.NotIn)
    def not_in(self, interp, frame, stmt) -> Result[types.TypeAttribute]:
        return (types.Bool,)

    @impl(py.NewList)
    def new_list(
        self, interp, frame: Frame[types.TypeAttribute], stmt: py.NewList
    ) -> Result[types.TypeAttribute]:
        values = frame.get_values(stmt.values)
        if not values:
            return (types.List[types.Any],)

        elem = values[0]
        for typ in values[1:]:
            elem = elem.join(typ)

        if isinstance(elem, types.TypeAttribute):
            return (types.List[elem],)
        return (stmt.result.type,)

    @impl(py.Slice)
    def slice(
        self, interp, frame: Frame[types.TypeAttribute], stmt: py.Slice
    ) -> Result[types.TypeAttribute]:
        start, stop, step = frame.get_values(stmt.args)
        if (
            isinstance(start, types.Const)
            and isinstance(stop, types.Const)
            and isinstance(step, types.Const)
            and isinstance(stmt.result.type, types.TypeAttribute)
        ):
            return (
                types.Const(slice(start.data, stop.data, step.data), stmt.result.type),
            )

        return (stmt.result.type,)
