import ast

from kirin import lowering2, exceptions

from . import stmts
from ._dialect import dialect


@dialect.register
class Lowering(lowering2.FromPythonAST):

    def lower_BinOp(self, state: lowering2.State, node: ast.BinOp) -> lowering2.Result:
        lhs = state.lower(node.left).expect_one()
        rhs = state.lower(node.right).expect_one()

        if op := getattr(stmts, node.op.__class__.__name__, None):
            stmt = op(lhs=lhs, rhs=rhs)
        else:
            raise exceptions.DialectLoweringError(f"unsupported binop {node.op}")
        return state.current_frame.push(stmt)
