from typing import Tuple

from kirin.dialects.py import types
from kirin.interp import DialectInterpreter, ResultValue, impl
from kirin.interp.base import BaseInterpreter
from kirin.interp.value import Result
from kirin.ir.nodes.stmt import Statement

from . import _stmts as py
from .dialect import dialect


@dialect.register(key="typeinfer")
class TypeInfer(DialectInterpreter):
    @classmethod  # just failsafe
    def fallback(
        cls, interp: BaseInterpreter, stmt: Statement, values: Tuple
    ) -> Result:
        return ResultValue(*tuple(types.Any for _ in stmt.results))

    # NOTE: const always contains the acutal value, so we can just return the type
    @impl(py.Constant)
    def constant(self, interp, stmt: py.Constant, values: tuple) -> ResultValue:
        return ResultValue(stmt.result.type)

    @impl(py.Alias)
    def alias(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0])  # just forward the type

    @impl(py.NewTuple)
    def new_tuple(self, interp, stmt, values: tuple[types.PyType, ...]) -> ResultValue:
        return ResultValue(types.Tuple.where(values))  # make 3.10 happy

    @impl(py.Add, types.Float, types.Float)
    @impl(py.Add, types.Float, types.Int)
    @impl(py.Add, types.Int, types.Float)
    def addf(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Float)

    @impl(py.Add, types.Int, types.Int)
    def addi(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Int)

    @impl(py.Sub, types.Float, types.Float)
    @impl(py.Sub, types.Float, types.Int)
    @impl(py.Sub, types.Int, types.Float)
    def subf(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Float)

    @impl(py.Sub, types.Int, types.Int)
    def subi(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Int)

    @impl(py.Sub, types.Float, types.Float)
    @impl(py.Sub, types.Float, types.Int)
    @impl(py.Sub, types.Int, types.Float)
    def multf(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Float)

    @impl(py.Mult, types.Int, types.Int)
    def multi(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Int)

    @impl(py.Div)
    def divf(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Float)

    @impl(py.Mod, types.Float, types.Float)
    @impl(py.Mod, types.Float, types.Int)
    @impl(py.Mod, types.Int, types.Float)
    def modf(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Float)

    @impl(py.Mod, types.Int, types.Int)
    def modi(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Int)

    @impl(py.UAdd)
    def uadd(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0])

    @impl(py.USub)
    def usub(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(values[0])

    @impl(py.Eq)
    def eq(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Bool)

    @impl(py.NotEq)
    def not_eq(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Bool)

    @impl(py.Lt)
    def lt(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Bool)

    @impl(py.LtE)
    def lt_eq(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Bool)

    @impl(py.Gt)
    def gt(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Bool)

    @impl(py.GtE)
    def gt_eq(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Bool)

    @impl(py.And)
    def and_(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Bool)

    @impl(py.Or)
    def or_(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Bool)

    @impl(py.Not)
    def not_(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Bool)

    @impl(py.BitAnd, types.Int, types.Int)
    def bit_andi(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Int)

    @impl(py.BitAnd, types.Bool, types.Bool)
    def bit_andb(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Bool)

    @impl(py.BitOr, types.Int, types.Int)
    def bit_ori(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Int)

    @impl(py.BitOr, types.Bool, types.Bool)
    def bit_orb(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Bool)

    @impl(py.BitXor, types.Int, types.Int)
    def bit_xori(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Int)

    @impl(py.BitXor, types.Bool, types.Bool)
    def bit_xorb(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Bool)

    @impl(py.Invert, types.Int)
    def invert(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Int)

    @impl(py.LShift, types.Int)
    def lshift(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Int)

    @impl(py.RShift, types.Int)
    def rshift(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Int)

    @impl(py.FloorDiv, types.Float, types.Float)
    @impl(py.FloorDiv, types.Int, types.Float)
    @impl(py.FloorDiv, types.Float, types.Int)
    def floor_divf(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Float)

    @impl(py.FloorDiv, types.Int, types.Int)
    def floor_divi(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Int)

    @impl(py.Pow, types.Float, types.Float)
    @impl(py.Pow, types.Float, types.Int)
    @impl(py.Pow, types.Int, types.Float)
    def powf(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Float)

    @impl(py.Pow, types.Int, types.Int)
    def powi(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Int)

    @impl(py.MatMult)
    def mat_mult(self, interp, stmt, values: tuple) -> ResultValue:
        raise NotImplementedError("np.array @ np.array not implemented")

    @impl(py.GetItem)
    def getitem(
        self, interp, stmt: py.GetItem, values: tuple[types.PyType, types.PyType]
    ) -> ResultValue:
        obj: types.PyGeneric = values[0]
        index: types.PyType = values[1]
        # TODO: replace this when we can multiple dispatch
        if obj.is_subseteq(types.Tuple):
            return self.getitem_tuple(interp, stmt, obj, index)
        elif obj.is_subseteq(types.List):
            if index.is_subseteq(types.Int):
                return ResultValue(obj.vars[0])
            elif index.is_subseteq(types.Slice):
                return ResultValue(obj)
            else:
                return ResultValue(types.Bottom)
        else:
            return ResultValue(types.Any)

    def getitem_tuple(
        self,
        interp,
        stmt: py.GetItem,
        obj: types.PyGeneric,
        index: types.PyType,
    ):
        if index.is_subseteq(types.Int):
            return self.getitem_tuple_index(interp, stmt, obj, index)
        elif index.is_subseteq(types.Slice):
            return self.getitem_tuple_slice(interp, stmt, obj, index)
        else:
            return ResultValue(types.Bottom)

    def getitem_tuple_index(
        self,
        interp,
        stmt: py.GetItem,
        obj: types.PyGeneric,
        index: types.PyType,
    ):
        if isinstance(index, types.PyConst):  # const
            if obj.vararg and index.data >= len(obj.vars):
                return ResultValue(obj.vararg.typ)
            elif index.data < len(obj.vars):
                return ResultValue(obj.vars[index.data])
            else:
                return ResultValue(types.Bottom)
        else:
            return ResultValue(self.getitem_tuple_union(obj))

    def getitem_tuple_slice(
        self,
        interp,
        stmt: py.GetItem,
        obj: types.PyGeneric,
        index: types.PyType,
    ):
        if isinstance(index, types.PyConst):
            data: slice = index.data
            if obj.vararg and data.stop >= len(obj.vars):
                return ResultValue(
                    types.PyUnion(
                        *obj.vars[slice(data.start, len(obj.vars), data.step)],
                        obj.vararg.typ,
                    )
                )
            elif data.stop is None or data.stop < len(obj.vars):
                return ResultValue(
                    types.Tuple.where(obj.vars[slice(data.start, data.stop, data.step)])
                )
            else:  # out of bounds
                return ResultValue(types.Bottom)
        else:
            return ResultValue(
                types.Tuple[types.PyVararg(self.getitem_tuple_union(obj))]
            )

    def getitem_tuple_union(self, obj: types.PyGeneric):
        if obj.vararg:
            return types.PyUnion(*obj.vars, obj.vararg.typ)
        else:
            return types.PyUnion(*obj.vars)

    @impl(py.Abs, types.Int)
    def absi(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Int)

    @impl(py.Abs, types.Float)
    def absf(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Float)

    @impl(py.SetItem)
    def setindex(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.NoneType)

    @impl(py.Is)
    def is_(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Bool)

    @impl(py.IsNot)
    def is_not(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Bool)

    @impl(py.In)
    def in_(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Bool)

    @impl(py.NotIn)
    def not_in(self, interp, stmt, values: tuple) -> ResultValue:
        return ResultValue(types.Bool)

    @impl(py.NewList)
    def new_list(
        self, interp, stmt, values: tuple[types.PyType, ...]
    ) -> ResultValue[types.PyType]:
        elem = values[0]
        for typ in values[1:]:
            elem = elem.join(typ)  # type: ignore
        return ResultValue(types.List[elem])  # type: ignore

    @impl(py.Slice)
    def slice(self, interp, stmt: py.Slice, values: tuple) -> ResultValue:
        start, stop, step = values
        if (
            isinstance(start, types.PyConst)
            and isinstance(stop, types.PyConst)
            and isinstance(step, types.PyConst)
        ):
            return ResultValue(
                types.PyConst(slice(start.data, stop.data, step.data), stmt.result.type)
            )

        return ResultValue(stmt.result)
