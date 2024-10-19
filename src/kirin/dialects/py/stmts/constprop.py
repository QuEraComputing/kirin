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
            if index.data < 0 or index.data >= len(obj):
                return ResultValue(NotConst())
            return ResultValue(obj[index.data])
        return ResultValue(NotConst())
