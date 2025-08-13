from dataclasses import field, dataclass

from kirin import ir, passes, rewrite
from kirin.analysis import CallGraph
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.dialects.func.stmts import Invoke


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
            Invoke(
                inputs=node.inputs,
                callee=new_callee,
                purity=node.purity,
                kwargs=node.kwargs,
            )
        )

        return RewriteResult(has_done_something=True)


@dataclass
class CallGraphPass(passes.Pass):
    """Copy all functions in the call graph and apply a rule to each of them.


    Usage:
        rule = Walk(SomeRewriteRule())
        pass_ = CallGraphPass(rule=rule, dialects=...)
        pass_(some_method)

    Note: This pass modifies the input method in place, but copies
    all methods invoked within it before applying the rule to them.

    """

    rule: RewriteRule
    """The rule to apply to each function in the call graph."""

    fold_pass: passes.Fold = field(init=False)

    def __post_init__(self):
        self.fold_pass = passes.Fold(self.dialects, no_raise=self.no_raise)

    def unsafe_run(self, mt: ir.Method) -> RewriteResult:
        result = RewriteResult()
        mt_map = {}

        cg = CallGraph(mt)

        all_methods = set(sum(map(tuple, cg.defs.values()), ()))
        for original_mt in all_methods:
            if original_mt is mt:
                new_mt = original_mt
            else:
                new_mt = original_mt.similar()
            result = self.rule.rewrite(new_mt.code).join(result)
            mt_map[original_mt] = new_mt

        if result.has_done_something:
            for _, new_mt in mt_map.items():
                rewrite.Walk(ReplaceMethods(mt_map)).rewrite(new_mt.code)
                self.fold_pass(new_mt)

        return result
