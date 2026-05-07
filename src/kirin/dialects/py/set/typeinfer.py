from kirin import interp

from .stmts import New
from ._dialect import dialect


@dialect.register(key="typeinfer")
class TypeInfer(interp.MethodTable):

    @interp.impl(New)
    def new(self, interp, frame, stmt: New):
        return (stmt.result.type,)
