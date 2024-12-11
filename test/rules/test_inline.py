# type: ignore
from kirin import ir, types
from kirin.analysis import ConstProp, NotConst
from kirin.analysis.cfg import CFG
from kirin.decl import info, statement
from kirin.dialects.py import data, stmts
from kirin.prelude import basic_no_opt
from kirin.rewrite import Chain, Fixpoint, Walk
from kirin.rules.call2invoke import Call2Invoke
from kirin.rules.cfg_compactify import CFGCompactify
from kirin.rules.dce import DeadCodeElimination
from kirin.rules.fold import ConstantFold
from kirin.rules.getfield import InlineGetField
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
    assert len(inline_foldl.callable_region.blocks) == 1
    assert inline_foldl(2) == 6
    inline_foldl.print()


def test_inline_single_entry():
    dialect = ir.Dialect("dummy2")

    @statement(dialect=dialect)
    class DummyStmtWithSiteEffect(ir.Statement):
        name = "dummy2"
        value: ir.SSAValue = info.argument(types.Int)
        option: data.PyAttr[str] = info.attribute()
        # result: ir.ResultValue = info.result(types.Int)

    @basic_no_opt.add(dialect)
    def inline_npure(x: int, y: int):
        DummyStmtWithSiteEffect(x, option="attr")
        DummyStmtWithSiteEffect(y, option="attr2")

    @basic_no_opt.add(dialect)
    def inline_non_pure():
        DummyStmtWithSiteEffect(3, option="attr0")
        inline_npure(1, 2)

    inline_non_pure.code.print()
    inline = Inline(heuristic=lambda x: True)
    Walk(inline).rewrite(inline_non_pure.code)
    Fixpoint(CFGCompactify(CFG(inline_non_pure.callable_region))).rewrite(
        inline_non_pure.code
    )
    inline_non_pure.code.print()
    assert isinstance(
        inline_non_pure.callable_region.blocks[0].stmts.at(1), DummyStmtWithSiteEffect
    )
    assert isinstance(
        inline_non_pure.callable_region.blocks[0].stmts.at(5), DummyStmtWithSiteEffect
    )
    assert isinstance(
        inline_non_pure.callable_region.blocks[0].stmts.at(6), DummyStmtWithSiteEffect
    )


def test_inline_non_foldable_closure():
    dialect = ir.Dialect("dummy2")

    @statement(dialect=dialect)
    class DummyStmt2(ir.Statement):
        name = "dummy2"
        value: ir.SSAValue = info.argument(types.Int)
        option: data.PyAttr[str] = info.attribute()
        result: ir.ResultValue = info.result(types.Int)

    @basic_no_opt.add(dialect)
    def unfolable(x: int, y: int):
        def inner():
            DummyStmt2(x, option="hello")
            DummyStmt2(y, option="hello")

        return inner

    @basic_no_opt.add(dialect)
    def main():
        x = DummyStmt2(1, option="hello")
        x = unfolable(x, x)
        return x()

    main.print()
    inline = Walk(Inline(lambda _: True))
    inline.rewrite(main.code)
    constprop = ConstProp(basic_no_opt)
    constprop.eval(main, ())
    ConstantFold(constprop.results).rewrite(main.code)
    compact = Fixpoint(CFGCompactify(CFG(main.callable_region)))
    compact.rewrite(main.code)
    inline.rewrite(main.code)
    compact = Fixpoint(CFGCompactify(CFG(main.callable_region)))
    compact.rewrite(main.code)
    Fixpoint(Walk(InlineGetField())).rewrite(main.code)
    constprop = ConstProp(basic_no_opt)
    constprop.eval(main, ())
    Walk(DeadCodeElimination(constprop.results)).rewrite(main.code)
    main.print(analysis=constprop.results)

    @basic_no_opt.add(dialect)
    def target():
        x = DummyStmt2(1, option="hello")
        DummyStmt2(x, option="hello")
        DummyStmt2(x, option="hello")
        return

    CFGCompactify(CFG(target.callable_region)).rewrite(target.code)
    assert target.callable_region.is_structurally_equal(main.callable_region)
