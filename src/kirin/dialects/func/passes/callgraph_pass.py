from dataclasses import dataclass

from kirin import ir, passes
from kirin.rewrite import Walk
from kirin.rewrite.abc import RewriteRule, RewriteResult

from ..stmts import Invoke


@dataclass
class ReplaceMethods(RewriteRule):
    new_symbols: dict[ir.Method, ir.Method]

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if (
            not isinstance(node, Invoke)
            or (new_callee := self.new_symbols.get(node.callee)) is None
        ):
            return RewriteResult()

        node.replace_by(
            Invoke.from_stmt(node, attributes={"callee": ir.PyAttr(new_callee)})
        )

        return RewriteResult(has_done_something=True)


@dataclass
class CallGraphPass(passes.Pass):
    """Copy all functions in the call graph and apply a rule to each of them."""

    rule: RewriteRule
    """The rule to apply to each function in the call graph."""

    def methods_on_callgraph(self, mt: ir.Method) -> set[ir.Method]:

        callees = {mt}
        stack = [mt]

        while stack:
            current_mt = stack.pop()
            for stmt in current_mt.callable_region.walk():
                if isinstance(stmt, Invoke):
                    callee = stmt.callee
                    if callee not in callees:
                        callees.add(callee)
                        stack.append(callee)

        return callees

    def unsafe_run(self, mt: ir.Method) -> RewriteResult:
        result = RewriteResult()
        mt_map = {}

        subroutines = self.methods_on_callgraph(mt)
        for original_mt in subroutines:
            if original_mt is mt:
                new_mt = original_mt
            else:
                new_mt = original_mt.similar()
            result = self.rule.rewrite(new_mt.code).join(result)
            mt_map[original_mt] = new_mt

        if result.has_done_something:
            for _, new_mt in mt_map.items():
                Walk(ReplaceMethods(mt_map)).rewrite(new_mt.code)
                passes.Fold(self.dialects)(new_mt)

        return result
