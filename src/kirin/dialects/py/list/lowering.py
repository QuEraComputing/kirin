import ast

from kirin import types, lowering

from .stmts import New
from ._dialect import dialect


@dialect.register
class PythonLowering(lowering.FromPythonAST):

    def lower_List(self, state: lowering.State, node: ast.List) -> lowering.Result:
        elts = tuple(state.lower(each).expect_one() for each in node.elts)

        if len(elts):
            typ = elts[0].type
            for each in elts:
                typ = typ.join(each.type)
        else:
            typ = types.Any

        return state.current_frame.push(New(values=tuple(elts)))
