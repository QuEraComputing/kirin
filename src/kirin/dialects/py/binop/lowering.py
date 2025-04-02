import ast

from kirin import lowering

from . import stmts
from ._dialect import dialect


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_BinOp(self, state: lowering.State, node: ast.BinOp) -> lowering.Result:
        lhs = state.lower(node.left).expect_one()
        rhs = state.lower(node.right).expect_one()

        if op := getattr(stmts, node.op.__class__.__name__, None):
            stmt = op(lhs=lhs, rhs=rhs)
        else:
            raise lowering.BuildError(f"unsupported binop {node.op}")
        return state.current_frame.push(stmt)
