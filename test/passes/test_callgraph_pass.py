from kirin import ir
from kirin.decl import statement
from kirin.prelude import structural_no_opt
from kirin.rewrite import abc
from kirin.dialects import func
from kirin.passes.callgraph import CallGraphPass
from kirin.analysis.callgraph import CallGraph


def test_callgraph_pass():

    dialect = ir.Dialect("test")

    @statement(dialect=dialect)
    class TestStmt(ir.Statement):
        pass

    class RewriteRule(abc.RewriteRule):
        def rewrite_Statement(self, node: ir.Statement) -> abc.RewriteResult:
            if not isinstance(node, func.Function):
                return abc.RewriteResult()

            first_stmt = node.body.blocks[0].first_stmt
            assert first_stmt is not None
            TestStmt().insert_before(first_stmt)

            return abc.RewriteResult(has_done_something=True)

    @structural_no_opt
    def subroutine(x):
        if x < 10:
            return subroutine(x + 1)
        else:
            return x

    @structural_no_opt
    def main():
        return subroutine(0)

    rule = RewriteRule()
    pass_ = CallGraphPass(rule=rule, dialects=structural_no_opt)

    pass_(main)

    cg = CallGraph(main)

    for mts in cg.defs.values():
        for mt in mts:
            assert isinstance(mt.callable_region.blocks[0].stmts.at(0), TestStmt)
