from kirin.analysis.dataflow.constprop import (
    Const,
    ConstProp,
    ConstPropLattice,
    NotConst,
    PartialTuple,
)
from kirin.interp import DialectInterpreter, ResultValue, impl

from . import _stmts as py
from .dialect import dialect


@dialect.register(key="constprop")
class DialectConstProp(DialectInterpreter):

    @impl(py.NewTuple)
    def new_tuple(
        self, interp: ConstProp, stmt: py.NewTuple, values: tuple[ConstPropLattice, ...]
    ) -> ResultValue:
        return ResultValue(PartialTuple(values))

    @impl(py.Not)
    def not_(self, interp, stmt: py.Not, values: tuple) -> ResultValue:
        if isinstance(stmt.value.owner, py.NewTuple):
            return ResultValue(Const(len(stmt.value.owner.args) == 0))
        elif isinstance(values[0], Const):
            return ResultValue(Const(not values[0].data))
        return ResultValue(NotConst())

    @impl(py.GetItem)
    def getitem(
        self,
        interp,
        stmt: py.GetItem,
        values: tuple[ConstPropLattice, ConstPropLattice],
    ) -> ResultValue:
        obj = values[0]
        index = values[1]
        if not isinstance(index, Const):
            return ResultValue(NotConst())

        if isinstance(obj, PartialTuple):
            obj = obj.data
            if isinstance(index.data, int) and 0 <= index.data < len(obj):
                return ResultValue(obj[index.data])
            elif isinstance(index.data, slice):
                start, stop, step = index.data.indices(len(obj))
                return ResultValue(PartialTuple(obj[start:stop:step]))
        return ResultValue(NotConst())
