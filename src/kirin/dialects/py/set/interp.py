from kirin import interp

from .stmts import New
from ._dialect import dialect


@dialect.register
class SetMethods(interp.MethodTable):

    @interp.impl(New)
    def new(self, interp, frame: interp.Frame, stmt: New):
        return (set(frame.get_values(stmt.values)),)
