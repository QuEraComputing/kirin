from dataclasses import dataclass

from kirin import ir, types
from kirin.dialects import py
from kirin.rewrite.abc import RewriteRule, RewriteResult


@dataclass
class PeepholeOptimize(RewriteRule):

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        result_types = node.results.types
        if not all(
            t.is_subseteq(types.Union(types.Float, types.Int)) for t in result_types
        ):
            return RewriteResult(has_done_something=False)

        if isinstance(node, py.binop.Add):
            #   add(%a, %a) -> mul(2, %a)
            if node.lhs is node.rhs:
                x = py.Constant(2)
                x.insert_before(node)
                new_stmt = py.binop.Mult(x.result, node.rhs)
                node.replace_by(new_stmt)
                return RewriteResult(has_done_something=True)

            #   add(mul(2, %a), %a) -> mul(3, %a)
            elif isinstance(mult_node := node.lhs.owner, py.binop.Mult) and isinstance(
                const_node := mult_node.lhs.owner, py.Constant
            ):
                x = const_node.value.unwrap()
                const_node.replace_by(py.Constant(x + 1))
                node.replace_by(py.binop.Mult(mult_node.lhs, node.rhs))
                mult_node.delete()
                return RewriteResult(has_done_something=True)

            #   add(%a, mul(2, %a)) -> mul(3, %a)
            elif isinstance(mult_node := node.rhs.owner, py.binop.Mult) and isinstance(
                const_node := mult_node.lhs.owner, py.Constant
            ):
                x = const_node.value.unwrap()
                const_node.replace_by(py.Constant(x + 1))
                node.replace_by(py.binop.Mult(mult_node.lhs, node.lhs))
                mult_node.delete()
                return RewriteResult(has_done_something=True)

        return RewriteResult(has_done_something=False)
