# type: ignore
from kirin import ir, types
from kirin.decl import info, statement
from kirin.passes import Fold
from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk, Fixpoint, WrapConst
from kirin.analysis import const
from kirin.dialects.py import constant
from kirin.rewrite.dce import DeadCodeElimination
from kirin.rewrite.fold import ConstantFold
from kirin.rewrite.inline import Inline
from kirin.rewrite.getfield import InlineGetField
from kirin.rewrite.compactify import CFGCompactify

fold_pass = Fold(basic_no_opt)


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
    fold_pass.fixpoint(inline_closure)
    Walk(Inline(heuristic=lambda x: True)).rewrite(inline_closure.code)
    fold_pass.fixpoint(inline_closure)
    inline_closure.code.print()
    stmt = inline_closure.callable_region.blocks[0].stmts.at(0)
    assert isinstance(stmt, constant.Constant)
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
    for _ in range(5):
        Walk(Inline(heuristic=lambda x: True)).rewrite(inline_foldl.code)
        fold_pass.fixpoint(inline_foldl)
    # inline_foldl.print(hint="const")
    assert len(inline_foldl.callable_region.blocks) == 1
    assert inline_foldl(2) == 6


def test_inline_single_entry():
    dialect = ir.Dialect("dummy2")

    @statement(dialect=dialect)
    class DummyStmtWithSiteEffect(ir.Statement):
        name = "dummy2"
        traits = frozenset({ir.FromPythonCall()})
        value: ir.SSAValue = info.argument(types.Int)
        option: str = info.attribute()
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
    Fixpoint(CFGCompactify()).rewrite(inline_non_pure.code)
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
        traits = frozenset({ir.FromPythonCall()})
        value: ir.SSAValue = info.argument(types.Int)
        option: str = info.attribute()
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
    constprop = const.Propagate(basic_no_opt)
    frame, _ = constprop.run_analysis(main)
    Walk(Fixpoint(WrapConst(frame))).rewrite(main.code)
    ConstantFold().rewrite(main.code)
    compact = Fixpoint(CFGCompactify())
    compact.rewrite(main.code)
    inline.rewrite(main.code)
    compact = Fixpoint(CFGCompactify())
    compact.rewrite(main.code)
    Fixpoint(Walk(InlineGetField())).rewrite(main.code)
    constprop = const.Propagate(basic_no_opt)
    frame, ret = constprop.run_analysis(main, ())
    Walk(Fixpoint(WrapConst(frame))).rewrite(main.code)
    Walk(DeadCodeElimination()).rewrite(main.code)
    main.print(analysis=frame.entries)

    @basic_no_opt.add(dialect)
    def target():
        x = DummyStmt2(1, option="hello")
        DummyStmt2(x, option="hello")
        DummyStmt2(x, option="hello")
        return

    CFGCompactify().rewrite(target.code)
    assert target.callable_region.is_structurally_equal(main.callable_region)
