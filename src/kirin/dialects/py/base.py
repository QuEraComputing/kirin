"""Base dialect for Python.

This dialect does not contain statements. It only contains
lowering rules for `ast.Name` and `ast.Expr`.
"""

import ast

from kirin import ir, lowering2

dialect = ir.Dialect("py.base")


@dialect.register
class PythonLowering(lowering2.FromPythonAST):

    def lower_Name(self, state: lowering2.State, node: ast.Name) -> lowering2.Result:
        name = node.id
        if isinstance(node.ctx, ast.Load):
            value = state.current_frame.get(name)
            if value is None:
                raise lowering2.DialectLoweringError(f"{name} is not defined")
            return value
        elif isinstance(node.ctx, ast.Store):
            raise lowering2.DialectLoweringError("unhandled store operation")
        else:  # Del
            raise lowering2.DialectLoweringError("unhandled del operation")

    def lower_Expr(self, state: lowering2.State, node: ast.Expr) -> lowering2.Result:
        return state.parent.visit(state, node.value)
