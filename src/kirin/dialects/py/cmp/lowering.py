import ast

from kirin import ir, lowering
from kirin.dialects.py import boolop

from . import stmts
from ._dialect import dialect


@dialect.register
class PythonLowering(lowering.FromPythonAST):

    def lower_Compare(
        self, state: lowering.State, node: ast.Compare
    ) -> lowering.Result:
        # NOTE: a key difference here is we need to lower
        # the multi-argument comparison operators into binary operators
        # since low-level comparision operators are binary + we need a static
        # number of arguments in each instruction
        lhs = state.lower(node.left).expect_one()

        comparators = [
            state.lower(comparator).expect_one() for comparator in node.comparators
        ]

        cmp_results: list[ir.SSAValue] = []
        for op, rhs in zip(node.ops, comparators):
            if cls := getattr(stmts, op.__class__.__name__, None):
                stmt: stmts.Cmp = cls(lhs=lhs, rhs=rhs)
            else:
                raise lowering.BuildError(f"unsupported compare operator {op}")
            state.current_frame.push(stmt)
            cmp_results.append(stmt.result)
            lhs = rhs

        if len(cmp_results) == 1:
            return cmp_results[0]

        lhs = cmp_results[0]
        for rhs in cmp_results[1:]:
            and_stmt = boolop.And(lhs=lhs, rhs=rhs)
            state.current_frame.push(and_stmt)
            lhs = and_stmt.result

        return lhs
