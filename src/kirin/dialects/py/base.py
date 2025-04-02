"""Base dialect for Python.

This dialect does not contain statements. It only contains
lowering rules for `ast.Name` and `ast.Expr`.
"""

import ast

from kirin import ir, lowering

dialect = ir.Dialect("py.base")


@dialect.register
class PythonLowering(lowering.FromPythonAST):

    def lower_Name(self, state: lowering.State, node: ast.Name) -> lowering.Result:
        name = node.id
        if isinstance(node.ctx, ast.Load):
            value = state.current_frame.get(name)
            if value is None:
                raise lowering.DialectLoweringError(f"{name} is not defined")
            return value
        elif isinstance(node.ctx, ast.Store):
            raise lowering.DialectLoweringError("unhandled store operation")
        else:  # Del
            raise lowering.DialectLoweringError("unhandled del operation")

    def lower_Expr(self, state: lowering.State, node: ast.Expr) -> lowering.Result:
        return state.parent.visit(state, node.value)
