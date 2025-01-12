from kirin import interp
from kirin.analysis import const

from . import stmts
from ._dialect import dialect


@dialect.register(key="constprop")
class ConstProp(interp.MethodTable):

    @interp.impl(stmts.Not)
    def not_(
        self, _: const.Propagate, frame: interp.Frame, stmt: stmts.Not
    ) -> interp.StatementResult[const.JointResult]:
        from kirin.dialects.py import tuple

        if isinstance(stmt.value.owner, tuple.New):
            ret = const.Value(len(stmt.value.owner.args) == 0)
        elif isinstance(value := frame.get(stmt.value), const.Value):
            ret = const.Value(not value.data)
        else:
            ret = const.Unknown()
        return (const.JointResult(ret, const.Pure()),)
