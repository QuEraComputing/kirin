from kirin import ir, types, passes, lowering
from kirin.decl import info, statement
from kirin.prelude import basic_no_opt
from kirin.analysis import const
from kirin.dialects import py, func

new_dialect = ir.Dialect("test")


@statement(dialect=new_dialect)
class DefaultInit(ir.Statement):
    name = "test"

    traits = frozenset({lowering.FromPythonCall(), ir.Pure()})

    result: ir.ResultValue = info.result(types.Int)


dialect_group = basic_no_opt.add(new_dialect)


def test_fallback_try_eval_const_pure():
    @dialect_group
    def test():
        n = 10

        def _inner(val: int) -> int:
            return DefaultInit() * val

        return _inner(n)

    passes.HintConst(dialect_group)(test)

    const_n = test.callable_region.blocks[0].stmts.at(0)
    assert isinstance(const_n, py.Constant)
    assert const_n.result.hints.get("const") == const.Value(10)

    call_stmt = test.callable_region.blocks[0].stmts.at(2)
    assert isinstance(call_stmt, func.Call)
    assert isinstance(call_stmt.result.hints.get("const"), const.Unknown)
