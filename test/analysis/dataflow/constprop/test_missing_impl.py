from kirin import ir, types, passes, lowering
from kirin.decl import info, statement
from kirin.prelude import basic_no_opt
from kirin.analysis import const
from kirin.dialects import ilist

new_dialect = ir.Dialect("test")


@statement(dialect=new_dialect)
class DefaultInit(ir.Statement):
    name = "test"

    traits = frozenset({lowering.FromPythonCall(), ir.Pure()})

    result: ir.ResultValue = info.result(types.Int)


dialect_group = basic_no_opt.add(new_dialect)


def test_missing_impl_try_eval_const_pure():
    # this test is trying to trigger the code path in propagate.py
    # where a statement has no concrete implementation but is pure
    # in this case, the ilist will attempt to evaluate the closure
    # which contains a call to DefaultInit, which has no implementation
    # in the concrete interpreter. In this case we should still be able
    # to mark the result as Unknown, rather than failing the analysis.
    # In other words, if a statement has no implementation, but is pure,
    # the function `try_eval_const_pure` will catch the exception and
    # return Unknown for the result.
    @dialect_group
    def test():
        n = 10

        def _inner(val: int) -> int:
            return DefaultInit() * val  # type: ignore

        return ilist.map(_inner, ilist.range(n))

    passes.HintConst(dialect_group)(test)

    for i in range(5):
        stmt = test.callable_region.blocks[0].stmts.at(i)
        assert all(
            isinstance(result.hints.get("const"), const.Value)
            for result in stmt.results
        )

    call_stmt = test.callable_region.blocks[0].stmts.at(5)
    assert isinstance(call_stmt, ilist.Map)
    assert isinstance(call_stmt.result.hints.get("const"), const.Unknown)


test_missing_impl_try_eval_const_pure()
