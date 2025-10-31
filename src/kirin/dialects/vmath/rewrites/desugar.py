from kirin import ir
from kirin.dialects.py import Add, Div, Sub, Mult, BinOp
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.ir.attrs.types import Generic, PyClass
from kirin.dialects.ilist.runtime import IList

from ..stmts import add as vadd, div as vdiv, mul as vmul, sub as vsub
from .._dialect import dialect


@dialect.post_inference
class DesugarBinOp(RewriteRule):

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        match node:
            case BinOp():
                match (node.lhs.type, node.rhs.type):
                    case (PyClass(lhs_typ), Generic(PyClass(rhs_typ))):
                        if (lhs_typ is float or lhs_typ is int) and rhs_typ == IList:
                            return self.replace_binop(node)
                    case (Generic(PyClass(lhs_typ)), PyClass(rhs_typ)):
                        if lhs_typ is IList and (rhs_typ is float or rhs_typ is int):
                            return self.replace_binop(node)
                    case _:
                        return RewriteResult()

            case _:
                return RewriteResult()

    def replace_binop(self, node: ir.Statement):
        match node:
            case Add():
                node.replace_by(vadd(lhs=node.lhs, rhs=node.rhs))
                return RewriteResult(has_done_something=True)
            case Sub():
                node.replace_by(vsub(lhs=node.lhs, rhs=node.rhs))
                return RewriteResult(has_done_something=True)
            case Mult():
                node.replace_by(vmul(lhs=node.lhs, rhs=node.rhs))
                return RewriteResult(has_done_something=True)
            case Div():
                node.replace_by(vdiv(lhs=node.lhs, rhs=node.rhs))
                return RewriteResult(has_done_something=True)
            case _:
                return RewriteResult()
