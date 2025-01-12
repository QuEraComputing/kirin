import ast

from kirin import lowering, exceptions

from . import stmts
from ._dialect import dialect


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_UnaryOp(
        self, state: lowering.LoweringState, node: ast.UnaryOp
    ) -> lowering.Result:
        if op := getattr(stmts, node.op.__class__.__name__, None):
            return lowering.Result(
                state.append_stmt(op(state.visit(node.operand).expect_one()))
            )
        else:
            raise exceptions.DialectLoweringError(
                f"unsupported unary operator {node.op}"
            )
