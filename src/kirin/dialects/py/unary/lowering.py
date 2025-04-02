import ast

from kirin import lowering

from . import stmts
from ._dialect import dialect


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_UnaryOp(
        self, state: lowering.State, node: ast.UnaryOp
    ) -> lowering.Result:
        if op := getattr(stmts, node.op.__class__.__name__, None):
            return state.current_frame.push(op(state.lower(node.operand).expect_one()))
        else:
            raise lowering.BuildError(f"unsupported unary operator {node.op}")
