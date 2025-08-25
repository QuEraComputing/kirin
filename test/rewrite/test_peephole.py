from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk, Fixpoint
from kirin.rewrite.peephole import PeepholeOptimize


# add(%a, %a) -> mul(2, %a)
@basic_no_opt
def peephole1(a: int):
    x = a + a
    return x


# add(mul(2, %a), %a) -> mul(3, %a)
@basic_no_opt
def peephole2(a: int):
    x = (2 * a) + a
    return x


# add(%a, mul(2, %a)) -> mul(3, %a)
@basic_no_opt
def peephole3(a: int):
    x = a + (2 * a)
    return x


def aux(program):
    before = program(5)
    Fixpoint(Walk(PeepholeOptimize())).rewrite(program.code)
    after = program(5)
    assert before == after


def test_peephole_opt1():
    aux(peephole1)


def test_peephole_opt2():
    aux(peephole2)


def test_peephole_opt3():
    aux(peephole3)
