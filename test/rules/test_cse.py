from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk, Fixpoint
from kirin.rules.cse import CommonSubexpressionElimination


@basic_no_opt
def badprogram(x: int, y: int) -> int:
    a = x + y
    b = x + y
    x = a + b
    y = a + b
    return x + y


def test_cse():
    before = badprogram(1, 2)
    cse = CommonSubexpressionElimination()
    Fixpoint(Walk(cse)).rewrite(badprogram.code)
    after = badprogram(1, 2)

    assert before == after


@basic_no_opt
def cse_constant():
    x = 1
    y = 2
    z = 1
    return x + y + z


def test_cse_constant():
    # NOTE: issue #61
    before = cse_constant()
    cse_constant.print()
    cse = CommonSubexpressionElimination()
    Fixpoint(Walk(cse)).rewrite(cse_constant.code)
    after = cse_constant()
    cse_constant.print()
    assert before == after
    assert len(cse_constant.callable_region.blocks[0].stmts) == 5
