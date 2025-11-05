from kirin import ir, types
from kirin.rewrite import Walk
from kirin.dialects.py import Add, Div, Sub, Mult, BinOp
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.ir.nodes.base import IRNode
from kirin.dialects.ilist import IListType

from ..stmts import add as vadd, div as vdiv, mul as vmul, sub as vsub


class DesugarBinOp(RewriteRule):
    """
    Convert py.BinOp statements with one scalar arg and one IList arg
    to the corresponding vmath binop. Currently supported binops are
    add, mult, sub, and div. BinOps where both args are IList are not
    supported, since `+` between two IList objects is taken to mean
    concatenation.
    """

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        match node:
            case BinOp():
                if (
                    node.lhs.type.is_subseteq(types.Number)
                    and node.rhs.type.is_subseteq(IListType)
                ) or (
                    node.lhs.type.is_subseteq(IListType)
                    and node.rhs.type.is_subseteq(types.Number)
                ):
                    return self.replace_binop(node)

            case _:
                return RewriteResult()

        return RewriteResult()

    def replace_binop(self, node: ir.Statement) -> RewriteResult:
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


class WalkDesugarBinop(RewriteRule):
    """
    Walks DesugarBinop. Needed for correct behavior when
    registering as a post-inference rewrite.
    """

    def rewrite(self, node: IRNode):
        return Walk(DesugarBinOp()).rewrite(node)
