from kirin.analysis import ConstProp, NotConst
from kirin.analysis.cfg import CFG
from kirin.dialects.py import stmts
from kirin.prelude import basic_no_opt
from kirin.rewrite import Chain, Fixpoint, Walk
from kirin.rules.call2invoke import Call2Invoke
from kirin.rules.cfg_compatify import CFGCompactify
from kirin.rules.dce import DeadCodeElimination
from kirin.rules.fold import ConstantFold
from kirin.rules.getitem import InlineGetItem
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
                    DeadCodeElimination(constprop.results),
                ]
            )
        )
    ).rewrite(inline_closure.code)
    Walk(Inline(heuristic=lambda x: True)).rewrite(inline_closure.code)
    cfg = CFG(inline_closure.callable_region)
    compactify = CFGCompactify(cfg)
    Fixpoint(compactify).rewrite(inline_closure.code)
    Fixpoint(Walk(DeadCodeElimination(constprop.results))).rewrite(inline_closure.code)
    inline_closure.code.print()
    stmt = inline_closure.callable_region.blocks[0].stmts.at(0)
    assert isinstance(stmt, stmts.Constant)
    assert inline_closure() == 40


@basic_no_opt
def add(x, y):
    return x + y


@basic_no_opt
def foldl(f, acc, xs: tuple):
    if not xs:
        return acc
    ret = foldl(f, acc, xs[1:])
    return f(ret, xs[0])


@basic_no_opt
def inline_foldl(x):
    return foldl(add, 0, (x, x, x))


def test_inline_constprop():
    def fold():
        constprop = ConstProp(inline_foldl.dialects)
        constprop.eval(inline_foldl, tuple(NotConst() for _ in inline_foldl.args))
        Fixpoint(
            Walk(
                Chain(
                    [
                        ConstantFold(constprop.results),
                        InlineGetItem(constprop.results),
                        Call2Invoke(constprop.results),
                        DeadCodeElimination(constprop.results),
                    ]
                )
            )
        ).rewrite(inline_foldl.code)
        compactify = Fixpoint(CFGCompactify(CFG(inline_foldl.callable_region)))
        compactify.rewrite(inline_foldl.code)
        Fixpoint(Walk(DeadCodeElimination(constprop.results))).rewrite(
            inline_foldl.code
        )

    Walk(Inline(heuristic=lambda x: True)).rewrite(inline_foldl.code)
    fold()
    Walk(Inline(heuristic=lambda x: True)).rewrite(inline_foldl.code)
    fold()
    Walk(Inline(heuristic=lambda x: True)).rewrite(inline_foldl.code)
    fold()
    Walk(Inline(heuristic=lambda x: True)).rewrite(inline_foldl.code)
    fold()
    Walk(Inline(heuristic=lambda x: True)).rewrite(inline_foldl.code)
    fold()
    Walk(Inline(heuristic=lambda x: True)).rewrite(inline_foldl.code)
    fold()
    inline_foldl.code.print()

    assert len(inline_foldl.callable_region.blocks) == 1
    assert inline_foldl(2) == 6
    inline_foldl.print()
