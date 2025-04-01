import ast

from kirin import lowering2

from . import stmts
from ._dialect import dialect


@dialect.register
class Lowering(lowering2.FromPythonAST):

    def lower_UnaryOp(
        self, state: lowering2.State, node: ast.UnaryOp
    ) -> lowering2.Result:
        if op := getattr(stmts, node.op.__class__.__name__, None):
            return state.current_frame.push(op(state.lower(node.operand).expect_one()))
        else:
            raise lowering2.DialectLoweringError(
                f"unsupported unary operator {node.op}"
            )
