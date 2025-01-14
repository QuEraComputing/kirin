"""Base dialect for Python.

This dialect does not contain statements. It only contains
lowering rules for `ast.Name` and `ast.Expr`.
"""

import ast

from kirin import ir, lowering, exceptions

dialect = ir.Dialect("py.base")


@dialect.register
class PythonLowering(lowering.FromPythonAST):

    def lower_Name(
        self, state: lowering.LoweringState, node: ast.Name
    ) -> lowering.Result:
        name = node.id
        if isinstance(node.ctx, ast.Load):
            value = state.current_frame.get(name)
            if value is None:
                raise exceptions.DialectLoweringError(f"{name} is not defined")
            return lowering.Result(value)
        elif isinstance(node.ctx, ast.Store):
            raise exceptions.DialectLoweringError("unhandled store operation")
        else:  # Del
            raise exceptions.DialectLoweringError("unhandled del operation")

    def lower_Expr(
        self, state: lowering.LoweringState, node: ast.Expr
    ) -> lowering.Result:
        return state.visit(node.value)
