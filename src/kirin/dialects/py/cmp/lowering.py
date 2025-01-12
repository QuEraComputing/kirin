import ast

from kirin.ir import SSAValue
from kirin.lowering import Result, FromPythonAST, LoweringState
from kirin.exceptions import DialectLoweringError
from kirin.dialects.py import boolop

from . import stmts as cmp
from ._dialect import dialect


@dialect.register
class PythonLowering(FromPythonAST):

    def lower_Compare(self, state: LoweringState, node: ast.Compare) -> Result:
        # NOTE: a key difference here is we need to lower
        # the multi-argument comparison operators into binary operators
        # since low-level comparision operators are binary + we need a static
        # number of arguments in each instruction
        lhs = state.visit(node.left).expect_one()

        comparators = [
            state.visit(comparator).expect_one() for comparator in node.comparators
        ]

        cmp_results: list[SSAValue] = []
        for op, rhs in zip(node.ops, comparators):
            if op := getattr(cmp, op.__class__.__name__, None):
                stmt = op(lhs=lhs, rhs=rhs)
            else:
                raise DialectLoweringError(f"unsupported compare operator {op}")
            state.append_stmt(stmt)
            cmp_results.append(Result(stmt).expect_one())
            lhs = rhs

        if len(cmp_results) == 1:
            return Result(cmp_results)

        lhs = cmp_results[0]
        for op in cmp_results[1:]:
            stmt = boolop.And(lhs=lhs, rhs=op)
            state.append_stmt(stmt)
            lhs = stmt.result

        return Result(lhs)
