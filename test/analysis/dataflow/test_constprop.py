from kirin import ir
from kirin.analysis.dataflow.constprop import (
    Const,
    ConstProp,
    ConstPropBottom,
    NotConst,
    NotPure,
    PartialTuple,
)
from kirin.decl import info, statement
from kirin.dialects.py import stmts, types
from kirin.prelude import basic_no_opt


class TestLattice:

    def test_meet(self):
        assert NotConst().meet(NotConst()) == NotConst()
        assert NotConst().meet(ConstPropBottom()) == ConstPropBottom()
        assert NotConst().meet(Const(1)) == Const(1)
        assert NotConst().meet(
            PartialTuple((Const(1), ConstPropBottom()))
        ) == PartialTuple((Const(1), ConstPropBottom()))
        assert ConstPropBottom().meet(NotConst()) == ConstPropBottom()
        assert ConstPropBottom().meet(ConstPropBottom()) == ConstPropBottom()
        assert ConstPropBottom().meet(Const(1)) == ConstPropBottom()
        assert (
            ConstPropBottom().meet(PartialTuple((Const(1), ConstPropBottom())))
            == ConstPropBottom()
        )
        assert Const(1).meet(NotConst()) == Const(1)
        assert Const(1).meet(ConstPropBottom()) == ConstPropBottom()
        assert Const(1).meet(Const(1)) == Const(1)
        assert (
            Const(1).meet(PartialTuple((Const(1), ConstPropBottom())))
            == ConstPropBottom()
        )
        assert PartialTuple((Const(1), ConstPropBottom())).meet(
            NotConst()
        ) == PartialTuple((Const(1), ConstPropBottom()))
        assert (
            PartialTuple((Const(1), ConstPropBottom())).meet(ConstPropBottom())
            == ConstPropBottom()
        )
        assert (
            PartialTuple((Const(1), ConstPropBottom())).meet(Const(1))
            == ConstPropBottom()
        )
        assert PartialTuple((Const(1), ConstPropBottom())).meet(
            Const((1, 2))
        ) == PartialTuple((Const(1), ConstPropBottom()))
        assert PartialTuple((Const(1), ConstPropBottom())).meet(
            PartialTuple((Const(1), ConstPropBottom()))
        ) == PartialTuple((Const(1), ConstPropBottom()))

    def test_join(self):
        assert NotConst().join(NotConst()) == NotConst()
        assert NotConst().join(ConstPropBottom()) == NotConst()
        assert NotConst().join(Const(1)) == NotConst()
        assert (
            NotConst().join(PartialTuple((Const(1), ConstPropBottom()))) == NotConst()
        )
        assert ConstPropBottom().join(NotConst()) == NotConst()
        assert ConstPropBottom().join(ConstPropBottom()) == ConstPropBottom()
        assert ConstPropBottom().join(Const(1)) == Const(1)
        assert ConstPropBottom().join(
            PartialTuple((Const(1), ConstPropBottom()))
        ) == PartialTuple((Const(1), ConstPropBottom()))
        assert PartialTuple((Const(1), ConstPropBottom())).join(
            Const((1, 2))
        ) == PartialTuple((Const(1), Const(2)))
        assert Const(1).join(NotConst()) == NotConst()
        assert Const(1).join(ConstPropBottom()) == Const(1)
        assert Const(1).join(Const(1)) == Const(1)
        assert Const(1).join(Const(2)) == NotConst()
        assert Const(1).join(PartialTuple((Const(1), ConstPropBottom()))) == NotConst()

    def test_is_equal(self):
        assert NotConst().is_equal(NotConst())
        assert not NotConst().is_equal(ConstPropBottom())
        assert not NotConst().is_equal(Const(1))
        assert ConstPropBottom().is_equal(ConstPropBottom())
        assert not ConstPropBottom().is_equal(Const(1))
        assert Const(1).is_equal(Const(1))
        assert not Const(1).is_equal(Const(2))
        assert PartialTuple((Const(1), ConstPropBottom())).is_equal(
            PartialTuple((Const(1), ConstPropBottom()))
        )
        assert not PartialTuple((Const(1), ConstPropBottom())).is_equal(
            PartialTuple((Const(1), Const(2)))
        )

    def test_partial_tuple(self):
        pt1 = PartialTuple((Const(1), ConstPropBottom()))
        pt2 = PartialTuple((Const(1), ConstPropBottom()))
        assert pt1.is_equal(pt2)
        assert pt1.is_subseteq(pt2)
        assert pt1.join(pt2) == pt1
        assert pt1.meet(pt2) == pt1
        pt2 = PartialTuple((Const(1), Const(2)))
        assert not pt1.is_equal(pt2)
        assert pt1.is_subseteq(pt2)
        assert pt1.join(pt2) == PartialTuple((Const(1), Const(2)))
        assert pt1.meet(pt2) == PartialTuple((Const(1), ConstPropBottom()))
        pt2 = PartialTuple((Const(1), ConstPropBottom()))
        assert pt1.is_equal(pt2)
        assert pt1.is_subseteq(pt2)
        assert pt1.join(pt2) == pt1
        assert pt1.meet(pt2) == pt1
        pt2 = PartialTuple((Const(1), NotConst()))
        assert not pt1.is_equal(pt2)
        assert pt1.is_subseteq(pt2)
        assert pt1.join(pt2) == pt2
        assert pt1.meet(pt2) == pt1


@basic_no_opt
def foo(x):
    return x + 1


@basic_no_opt
def goo(x):
    return foo(2), foo(x)


@basic_no_opt
def main():
    return goo(3)


@basic_no_opt
def bar(x):
    return goo(x)[0]


@basic_no_opt
def ntuple(len: int):
    if len == 0:
        return ()
    return (0,) + ntuple(len - 1)


@basic_no_opt
def recurse():
    return ntuple(3)


def test_constprop():
    infer = ConstProp(basic_no_opt)
    assert infer.eval(main, tuple(NotConst() for _ in main.args)).expect() == Const(
        (3, 4)
    )
    assert len(infer.results) == 3

    assert infer.eval(
        goo, tuple(NotConst() for _ in goo.args)
    ).expect() == PartialTuple((Const(3), NotConst()))
    assert len(infer.results) == 6
    block: ir.Block = goo.code.body.blocks[0]  # type: ignore
    assert infer.results[block.stmts.at(1).results[0]] == Const(3)
    assert infer.results[block.stmts.at(2).results[0]] == NotConst()
    assert infer.results[block.stmts.at(3).results[0]] == PartialTuple(
        (Const(3), NotConst())
    )

    assert infer.eval(bar, tuple(NotConst() for _ in bar.args)).expect() == Const(3)

    assert (
        infer.eval(ntuple, tuple(NotConst() for _ in ntuple.args)).expect()
        == NotConst()
    )
    assert infer.eval(
        recurse, tuple(NotConst() for _ in recurse.args)
    ).expect() == Const((0, 0, 0))


@basic_no_opt
def myfunc(x1: int) -> int:
    return x1 * 2


@basic_no_opt
def _for_loop_test_constp(
    cntr: int,
    x: tuple,
    n_range: int,
):
    if cntr < n_range:
        pos = myfunc(cntr)
        x = x + (cntr, pos)
        return _for_loop_test_constp(
            cntr=cntr + 1,
            x=x,
            n_range=n_range,
        )
    else:
        return x


def test_issue_40():
    constprop = ConstProp(basic_no_opt)
    result = constprop.eval(
        _for_loop_test_constp, (Const(0), Const(()), Const(5))
    ).expect()
    assert isinstance(result, Const)
    assert result.data == _for_loop_test_constp(cntr=0, x=(), n_range=5)


dummy_dialect = ir.Dialect("dummy")


@statement(dialect=dummy_dialect)
class DummyStatement(ir.Statement):
    name = "dummy"


def test_intraprocedure_side_effect():

    @basic_no_opt.add(dummy_dialect)
    def side_effect_return_none():
        DummyStatement()

    @basic_no_opt.add(dummy_dialect)
    def side_effect_intraprocedure(cond: bool):
        if cond:
            return side_effect_return_none()
        else:
            x = (1, 2, 3)
            return x

    constprop = ConstProp(basic_no_opt.add(dummy_dialect))
    result = constprop.eval(
        side_effect_intraprocedure,
        tuple(NotConst() for _ in side_effect_intraprocedure.args),
    ).expect()
    new_tuple = (
        side_effect_intraprocedure.callable_region.blocks[2].stmts.at(3).results[0]
    )
    assert isinstance(result, NotPure)
    assert constprop.results[new_tuple] == Const((1, 2, 3))


def test_interprocedure_true_branch():
    @basic_no_opt.add(dummy_dialect)
    def side_effect_maybe_return_none(cond: bool):
        if cond:
            return
        else:
            DummyStatement()
            return

    @basic_no_opt.add(dummy_dialect)
    def side_effect_true_branch_const(cond: bool):
        if cond:
            return side_effect_maybe_return_none(cond)
        else:
            return cond

    constprop = ConstProp(basic_no_opt.add(dummy_dialect))
    result = constprop.eval(
        side_effect_true_branch_const,
        tuple(NotConst() for _ in side_effect_true_branch_const.args),
    ).expect()
    assert isinstance(result, NotConst)  # instead of NotPure
    true_branch = side_effect_true_branch_const.callable_region.blocks[1]
    assert constprop.results[true_branch.stmts.at(0).results[0]] == Const(None)


def test_non_pure_recursion():
    @basic_no_opt
    def for_loop_append(cntr: int, x: list, n_range: int):
        if cntr < n_range:
            stmts.Append(x, cntr)  # type: ignore
            for_loop_append(cntr + 1, x, n_range)

        return x

    constprop = ConstProp(basic_no_opt)
    constprop.eval(for_loop_append, tuple(NotConst() for _ in for_loop_append.args))
    stmt = for_loop_append.callable_region.blocks[1].stmts.at(3)
    assert isinstance(constprop.results[stmt.results[0]], NotPure)


def test_closure_prop():
    dialect = ir.Dialect("dummy2")

    @statement(dialect=dialect)
    class DummyStmt2(ir.Statement):
        name = "dummy2"
        value: ir.SSAValue = info.argument(types.Int)
        result: ir.ResultValue = info.result(types.Int)

    @basic_no_opt.add(dialect)
    def non_const_closure(x: int, y: int):
        def inner():
            if False:
                return x + y
            else:
                return 2

        return inner

    @basic_no_opt.add(dialect)
    def non_pure(x: int, y: int):
        def inner():
            if False:
                return x + y
            else:
                DummyStmt2(1)  # type: ignore
                return 2

        return inner

    @basic_no_opt.add(dialect)
    def main():
        x = DummyStmt2(1)  # type: ignore
        x = non_const_closure(x, x)  # type: ignore
        return x()

    constprop = ConstProp(basic_no_opt.add(dialect))
    constprop.eval(main, ())
    main.print(analysis=constprop.results)
    stmt = main.callable_region.blocks[0].stmts.at(3)
    call_result = constprop.results[stmt.results[0]]
    assert isinstance(call_result, Const)
    assert call_result.data == 2

    @basic_no_opt.add(dialect)
    def main2():
        x = DummyStmt2(1)  # type: ignore
        x = non_pure(x, x)  # type: ignore
        return x()

    constprop = ConstProp(basic_no_opt.add(dialect))
    constprop.eval(main2, ())
    main2.print(analysis=constprop.results)
    stmt = main2.callable_region.blocks[0].stmts.at(3)
    call_result = constprop.results[stmt.results[0]]
    assert isinstance(call_result, NotPure)
