from kirin import ir
from kirin.analysis import const
from kirin.decl import info, statement
from kirin.dialects.py import stmts
from kirin.ir import types
from kirin.prelude import basic_no_opt


class TestLattice:

    def test_meet(self):
        assert const.NotConst().meet(const.NotConst()) == const.NotConst()
        assert const.NotConst().meet(const.Unknown()) == const.Unknown()
        assert const.NotConst().meet(const.Value(1)) == const.Value(1)
        assert const.NotConst().meet(
            const.PartialTuple((const.Value(1), const.Unknown()))
        ) == const.PartialTuple((const.Value(1), const.Unknown()))
        assert const.Unknown().meet(const.NotConst()) == const.Unknown()
        assert const.Unknown().meet(const.Unknown()) == const.Unknown()
        assert const.Unknown().meet(const.Value(1)) == const.Unknown()
        assert (
            const.Unknown().meet(const.PartialTuple((const.Value(1), const.Unknown())))
            == const.Unknown()
        )
        assert const.Value(1).meet(const.NotConst()) == const.Value(1)
        assert const.Value(1).meet(const.Unknown()) == const.Unknown()
        assert const.Value(1).meet(const.Value(1)) == const.Value(1)
        assert (
            const.Value(1).meet(const.PartialTuple((const.Value(1), const.Unknown())))
            == const.Unknown()
        )
        assert const.PartialTuple((const.Value(1), const.Unknown())).meet(
            const.NotConst()
        ) == const.PartialTuple((const.Value(1), const.Unknown()))
        assert (
            const.PartialTuple((const.Value(1), const.Unknown())).meet(const.Unknown())
            == const.Unknown()
        )
        assert (
            const.PartialTuple((const.Value(1), const.Unknown())).meet(const.Value(1))
            == const.Unknown()
        )
        assert const.PartialTuple((const.Value(1), const.Unknown())).meet(
            const.Value((1, 2))
        ) == const.PartialTuple((const.Value(1), const.Unknown()))
        assert const.PartialTuple((const.Value(1), const.Unknown())).meet(
            const.PartialTuple((const.Value(1), const.Unknown()))
        ) == const.PartialTuple((const.Value(1), const.Unknown()))

    def test_join(self):
        assert const.NotConst().join(const.NotConst()) == const.NotConst()
        assert const.NotConst().join(const.Unknown()) == const.NotConst()
        assert const.NotConst().join(const.Value(1)) == const.NotConst()
        assert (
            const.NotConst().join(const.PartialTuple((const.Value(1), const.Unknown())))
            == const.NotConst()
        )
        assert const.Unknown().join(const.NotConst()) == const.NotConst()
        assert const.Unknown().join(const.Unknown()) == const.Unknown()
        assert const.Unknown().join(const.Value(1)) == const.Value(1)
        assert const.Unknown().join(
            const.PartialTuple((const.Value(1), const.Unknown()))
        ) == const.PartialTuple((const.Value(1), const.Unknown()))
        assert const.PartialTuple((const.Value(1), const.Unknown())).join(
            const.Value((1, 2))
        ) == const.PartialTuple((const.Value(1), const.Value(2)))
        assert const.Value(1).join(const.NotConst()) == const.NotConst()
        assert const.Value(1).join(const.Unknown()) == const.Value(1)
        assert const.Value(1).join(const.Value(1)) == const.Value(1)
        assert const.Value(1).join(const.Value(2)) == const.NotConst()
        assert (
            const.Value(1).join(const.PartialTuple((const.Value(1), const.Unknown())))
            == const.NotConst()
        )

    def test_is_equal(self):
        assert const.NotConst().is_equal(const.NotConst())
        assert not const.NotConst().is_equal(const.Unknown())
        assert not const.NotConst().is_equal(const.Value(1))
        assert const.Unknown().is_equal(const.Unknown())
        assert not const.Unknown().is_equal(const.Value(1))
        assert const.Value(1).is_equal(const.Value(1))
        assert not const.Value(1).is_equal(const.Value(2))
        assert const.PartialTuple((const.Value(1), const.Unknown())).is_equal(
            const.PartialTuple((const.Value(1), const.Unknown()))
        )
        assert not const.PartialTuple((const.Value(1), const.Unknown())).is_equal(
            const.PartialTuple((const.Value(1), const.Value(2)))
        )

    def test_partial_tuple(self):
        pt1 = const.PartialTuple((const.Value(1), const.Unknown()))
        pt2 = const.PartialTuple((const.Value(1), const.Unknown()))
        assert pt1.is_equal(pt2)
        assert pt1.is_subseteq(pt2)
        assert pt1.join(pt2) == pt1
        assert pt1.meet(pt2) == pt1
        pt2 = const.PartialTuple((const.Value(1), const.Value(2)))
        assert not pt1.is_equal(pt2)
        assert pt1.is_subseteq(pt2)
        assert pt1.join(pt2) == const.PartialTuple((const.Value(1), const.Value(2)))
        assert pt1.meet(pt2) == const.PartialTuple((const.Value(1), const.Unknown()))
        pt2 = const.PartialTuple((const.Value(1), const.Unknown()))
        assert pt1.is_equal(pt2)
        assert pt1.is_subseteq(pt2)
        assert pt1.join(pt2) == pt1
        assert pt1.meet(pt2) == pt1
        pt2 = const.PartialTuple((const.Value(1), const.NotConst()))
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
    infer = const.Propagate(basic_no_opt)
    assert infer.eval(
        main, tuple(const.NotConst() for _ in main.args)
    ).expect() == const.Value((3, 4))
    assert len(infer.results) == 3

    assert infer.eval(
        goo, tuple(const.NotConst() for _ in goo.args)
    ).expect() == const.PartialTuple((const.Value(3), const.NotConst()))
    assert len(infer.results) == 6
    block: ir.Block = goo.code.body.blocks[0]  # type: ignore
    assert infer.results[block.stmts.at(1).results[0]] == const.Value(3)
    assert infer.results[block.stmts.at(2).results[0]] == const.NotConst()
    assert infer.results[block.stmts.at(3).results[0]] == const.PartialTuple(
        (const.Value(3), const.NotConst())
    )

    assert infer.eval(
        bar, tuple(const.NotConst() for _ in bar.args)
    ).expect() == const.Value(3)

    assert (
        infer.eval(ntuple, tuple(const.NotConst() for _ in ntuple.args)).expect()
        == const.NotConst()
    )
    assert infer.eval(
        recurse, tuple(const.NotConst() for _ in recurse.args)
    ).expect() == const.Value((0, 0, 0))


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
    constprop = const.Propagate(basic_no_opt)
    result = constprop.eval(
        _for_loop_test_constp, (const.Value(0), const.Value(()), const.Value(5))
    ).expect()
    assert isinstance(result, const.Value)
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

    constprop = const.Propagate(basic_no_opt.add(dummy_dialect))
    result = constprop.eval(
        side_effect_intraprocedure,
        tuple(const.NotConst() for _ in side_effect_intraprocedure.args),
    ).expect()
    new_tuple = (
        side_effect_intraprocedure.callable_region.blocks[2].stmts.at(3).results[0]
    )
    assert isinstance(result, const.NotConst)
    assert constprop.results[new_tuple] == const.Value((1, 2, 3))


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

    constprop = const.Propagate(basic_no_opt.add(dummy_dialect))
    result = constprop.eval(
        side_effect_true_branch_const,
        tuple(const.NotConst() for _ in side_effect_true_branch_const.args),
    ).expect()
    assert isinstance(result, const.NotConst)  # instead of NotPure
    true_branch = side_effect_true_branch_const.callable_region.blocks[1]
    assert constprop.results[true_branch.stmts.at(0).results[0]] == const.Value(None)


def test_non_pure_recursion():
    @basic_no_opt
    def for_loop_append(cntr: int, x: list, n_range: int):
        if cntr < n_range:
            stmts.Append(x, cntr)  # type: ignore
            for_loop_append(cntr + 1, x, n_range)

        return x

    constprop = const.Propagate(basic_no_opt)
    constprop.eval(
        for_loop_append, tuple(const.NotConst() for _ in for_loop_append.args)
    )
    stmt = for_loop_append.callable_region.blocks[1].stmts.at(3)
    assert isinstance(constprop.results[stmt.results[0]], const.NotConst)


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

    constprop = const.Propagate(basic_no_opt.add(dialect))
    constprop.eval(main, ())
    main.print(analysis=constprop.results)
    stmt = main.callable_region.blocks[0].stmts.at(3)
    call_result = constprop.results[stmt.results[0]]
    assert isinstance(call_result, const.Value)
    assert call_result.data == 2

    @basic_no_opt.add(dialect)
    def main2():
        x = DummyStmt2(1)  # type: ignore
        x = non_pure(x, x)  # type: ignore
        return x()

    constprop = const.Propagate(basic_no_opt.add(dialect))
    constprop.eval(main2, ())
    main2.print(analysis=constprop.results)
    stmt = main2.callable_region.blocks[0].stmts.at(3)
    call_result = constprop.results[stmt.results[0]]
    assert isinstance(call_result, const.Value)
