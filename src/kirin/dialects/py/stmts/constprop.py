from kirin import interp
from kirin.interp import MethodTable, impl
from kirin.analysis import const

from . import _stmts as py
from .dialect import dialect


@dialect.register(key="constprop")
class ConstPropTable(MethodTable):

    @impl(py.NewTuple)
    def new_tuple(
        self,
        _: const.Propagate,
        frame: interp.Frame[const.JointResult],
        stmt: py.NewTuple,
    ) -> interp.StatementResult[const.JointResult]:
        return (
            const.JointResult(
                const.PartialTuple(tuple(x.const for x in frame.get_values(stmt.args))),
                const.Pure(),
            ),
        )

    @impl(py.Not)
    def not_(
        self, _: const.Propagate, frame: interp.Frame, stmt: py.Not
    ) -> interp.StatementResult[const.JointResult]:
        if isinstance(stmt.value.owner, py.NewTuple):
            ret = const.Value(len(stmt.value.owner.args) == 0)
        elif isinstance(value := frame.get(stmt.value), const.Value):
            ret = const.Value(not value.data)
        else:
            ret = const.Unknown()
        return (const.JointResult(ret, const.Pure()),)

    @impl(py.GetItem)
    def getitem(
        self,
        _: const.Propagate,
        frame: interp.Frame[const.JointResult],
        stmt: py.GetItem,
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
