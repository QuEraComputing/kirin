import ast

from kirin import types
from kirin.lowering import Result, FromPythonAST, LoweringState

from .stmts import New
from ._dialect import dialect


@dialect.register
class PythonLowering(FromPythonAST):

    def lower_List(self, state: LoweringState, node: ast.List) -> Result:
        elts = tuple(state.visit(each).expect_one() for each in node.elts)

        if len(elts):
            typ = elts[0].type
            for each in elts:
                typ = typ.join(each.type)
        else:
            typ = types.Any

        stmt = New(values=tuple(elts))
        state.append_stmt(stmt)
        return Result(stmt)
