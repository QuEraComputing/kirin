from kirin import interp
from kirin.interp import DialectInterpreter, impl
from kirin.analysis import const

from . import _stmts as py
from .dialect import dialect


@dialect.register(key="constprop")
class DialectConstProp(DialectInterpreter):

    @impl(py.NewTuple)
    def new_tuple(
        self,
        interp: const.Propagate,
        stmt: py.NewTuple,
        values: tuple[const.JointResult, ...],
    ) -> interp.Result[const.JointResult]:
        return (
            const.JointResult(
                const.PartialTuple(tuple(x.const for x in values)),
                const.Pure(),
            ),
        )

    @impl(py.Not)
    def not_(
        self, interp, stmt: py.Not, values: tuple[const.JointResult, ...]
    ) -> interp.Result[const.JointResult]:
        if isinstance(stmt.value.owner, py.NewTuple):
            ret = const.Value(len(stmt.value.owner.args) == 0)
        elif isinstance(values[0], const.Value):
            ret = const.Value(not values[0].data)
        else:
            ret = const.Unknown()
        return (const.JointResult(ret, const.Pure()),)

    @impl(py.GetItem)
    def getitem(
        self,
        interp,
        stmt: py.GetItem,
        values: tuple[const.JointResult, const.JointResult],
    ) -> interp.Result[const.JointResult]:
        obj = values[0].const
        index = values[1].const
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
