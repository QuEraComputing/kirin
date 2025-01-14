import ast

from kirin import lowering, exceptions

from . import stmts
from ._dialect import dialect


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_BinOp(
        self, state: lowering.LoweringState, node: ast.BinOp
    ) -> lowering.Result:
        lhs = state.visit(node.left).expect_one()
        rhs = state.visit(node.right).expect_one()

        if op := getattr(stmts, node.op.__class__.__name__, None):
            stmt = op(lhs=lhs, rhs=rhs)
        else:
            raise exceptions.DialectLoweringError(f"unsupported binop {node.op}")
        return lowering.Result(state.append_stmt(stmt))
