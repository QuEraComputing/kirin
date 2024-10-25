from copy import copy
from dataclasses import dataclass

from kirin import ir
from kirin.dialects.py import stmts as py_stmts
from kirin.exceptions import DialectInterpretationError
from kirin.interp import Interpreter
from kirin.ir.nodes.stmt import Statement
from kirin.rewrite.abc import RewriteResult, RewriteRule


@dataclass
class AliasSimplifyRewrite(RewriteRule):
    interp: Interpreter

    def concrete_interp_const(self, p: ir.SSAValue):

        if not p.owner.has_trait(ir.ConstantLike):
            raise DialectInterpretationError("Invalid p, has to be constant")

        try:
            p_val = self.interp.run_stmt(p.owner, args=())

        except Exception as e:
            raise DialectInterpretationError(
                "Invalid concrete interp result value, has to be constant"
            ) from e

        return p_val.values[0]

    def rewrite_Statement(self, node: Statement) -> RewriteResult:
        match node:
            case py_stmts.Alias():
                return self.rewrite_alias(node)
            case _:
                return RewriteResult()

    def rewrite_alias(self, node: py_stmts.Alias) -> RewriteResult:

        for use in copy(node.result.uses):
            use.stmt.args[use.index] = node.value

        node.delete()

        return RewriteResult(has_done_something=True)


@dataclass
class GetitemSimplifyRewrite(RewriteRule):
    interp: Interpreter

    def concrete_interp_const(self, p: ir.SSAValue):

        if not p.owner.has_trait(ir.ConstantLike):
            raise DialectInterpretationError("Invalid p, has to be constant")

        try:
            p_val = self.interp.run_stmt(p.owner, args=())

        except Exception as e:
            raise DialectInterpretationError(
                "Invalid concrete interp result value, has to be constant"
            ) from e

        return p_val.values[0]

    def rewrite_Statement(self, node: Statement) -> RewriteResult:
        match node:
            case py_stmts.GetItem():
                return self.rewrite_getitem(node)

            case _:
                return RewriteResult()

    def rewrite_getitem(self, node: py_stmts.GetItem) -> RewriteResult:

        if not node.index.owner.has_trait(ir.ConstantLike):
            # do not rewrite because index is not constant.
            return RewriteResult()

        idx = self.concrete_interp_const(node.index)

        if isinstance(node.obj.owner, py_stmts.NewTuple):
            # tuple

            new_res = node.obj.owner.args[idx]

            for use in copy(node.result.uses):
                use.stmt.args[use.index] = new_res

            node.delete()
            return RewriteResult(has_done_something=True)
        else:
            return RewriteResult()
