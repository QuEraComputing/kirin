from kirin.prelude import basic_no_opt
from kirin.rewrite import Fixpoint, Walk
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
def ker_with_const():
    x = 0
    y = 0
    z = 1
    return (x, y, z + x)


def test_cse_const():
    before = ker_with_const()
    cse = CommonSubexpressionElimination()
    Fixpoint(Walk(cse)).rewrite(ker_with_const.code)
    after = ker_with_const()

    assert before == after
