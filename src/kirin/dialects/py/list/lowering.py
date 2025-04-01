import ast

from kirin import types, lowering2

from .stmts import New
from ._dialect import dialect


@dialect.register
class PythonLowering(lowering2.FromPythonAST):

    def lower_List(self, state: lowering2.State, node: ast.List) -> lowering2.Result:
        elts = tuple(state.lower(each).expect_one() for each in node.elts)

        if len(elts):
            typ = elts[0].type
            for each in elts:
                typ = typ.join(each.type)
        else:
            typ = types.Any

        return state.current_frame.push(New(values=tuple(elts)))
