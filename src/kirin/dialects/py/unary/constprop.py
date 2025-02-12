from kirin import interp
from kirin.analysis import const

from . import stmts
from ._dialect import dialect


@dialect.register(key="constprop")
class ConstProp(interp.MethodTable):

    @interp.impl(stmts.Not)
    def not_(
        self, _: const.Propagate, frame: const.Frame, stmt: stmts.Not
    ) -> interp.StatementResult[const.Result]:
        hint = frame.get(stmt.value)
        if isinstance(hint, (const.PartialTuple, const.Value)):
            ret = const.Value(not hint.data)
        else:
            ret = const.Unknown()
        return (ret,)
