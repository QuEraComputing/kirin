from kirin.analysis import ConstProp
from kirin.analysis.cfg import CFG
from kirin.dialects.py import stmts
from kirin.prelude import basic_no_opt
from kirin.rewrite import Chain, Fixpoint, Walk
from kirin.rules.call2invoke import Call2Invoke
from kirin.rules.cfg_compatify import CFGCompactify
from kirin.rules.dce import DeadCodeElimination
from kirin.rules.fold import ConstantFold
from kirin.rules.inline import Inline


@basic_no_opt
def somefunc(x: int):
    return x - 1


@basic_no_opt
def main(x: int):
    return somefunc(x) + 1


def test_simple():
    inline = Inline(heuristic=lambda x: True)
    a = main(1)
    main.code.print()
    Walk(inline).rewrite(main.code)
    main.code.print()
    b = main(1)
    assert a == b


@basic_no_opt
def closure_double(x: int, y: int):
    def foo(a: int, b: int):
        return a + b + x + y

    return foo


@basic_no_opt
def inline_closure():
    a = 3
    b = 4
    c = closure_double(1, 2)
    return c(a, b) * 4


def test_inline_closure():
    constprop = ConstProp(inline_closure.dialects)
    constprop.eval(inline_closure, ())
    Fixpoint(
        Walk(
            Chain(
                [
                    ConstantFold(constprop.results),
                    Call2Invoke(constprop.results),
                    DeadCodeElimination(),
                ]
            )
        )
    ).rewrite(inline_closure.code)
    Walk(Inline(heuristic=lambda x: True)).rewrite(inline_closure.code)
    cfg = CFG(inline_closure.callable_region)
    compactify = CFGCompactify(cfg)
    Fixpoint(compactify).rewrite(inline_closure.code)
    Fixpoint(Walk(DeadCodeElimination())).rewrite(inline_closure.code)
    inline_closure.code.print()
    stmt = inline_closure.callable_region.blocks[0].stmts.at(0)
    assert isinstance(stmt, stmts.Constant)
    assert inline_closure() == 40
