import ast

from kirin import lowering
from kirin.dialects.py.list._listcomp import lower_setcomp_via_desugaring

from .stmts import New
from ._dialect import dialect


@dialect.register
class PythonLowering(lowering.FromPythonAST):

    def lower_Set(self, state: lowering.State, node: ast.Set) -> lowering.Result:
        return state.current_frame.push(
            New(tuple(state.lower(each).expect_one() for each in node.elts))
        )

    def lower_SetComp(
        self, state: lowering.State, node: ast.SetComp
    ) -> lowering.Result:
        return lower_setcomp_via_desugaring(state, node)

    @lowering.akin(set)
    def lower_Call_set(self, state: lowering.State, node: ast.Call) -> lowering.Result:
        if len(node.args) != 0 or len(node.keywords) != 0:
            raise lowering.BuildError("`set(iterable)` is not supported yet")
        return state.current_frame.push(New(()))
