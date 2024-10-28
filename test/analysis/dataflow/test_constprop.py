from kirin import ir
from kirin.analysis.dataflow.constprop import (
    Const,
    ConstProp,
    ConstPropBottom,
    NotConst,
    NotPure,
    PartialTuple,
)
from kirin.decl import statement
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
    assert len(infer.results) == 4

    assert infer.eval(
        goo, tuple(NotConst() for _ in goo.args)
    ).expect() == PartialTuple((Const(3), NotConst()))
    assert len(infer.results) == 8
    block: ir.Block = goo.code.body.blocks[0]  # type: ignore
    assert infer.results[block.stmts.at(2).results[0]] == Const(3)
    assert infer.results[block.stmts.at(4).results[0]] == NotConst()
    assert infer.results[block.stmts.at(5).results[0]] == PartialTuple(
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


def test_intraprocedure_side_effect():
    dummy_dialect = ir.Dialect("dummy")

    @statement(dialect=dummy_dialect)
    class DummyStatement(ir.Statement):
        name = "dummy"

    @basic_no_opt.add(dummy_dialect)
    def side_effect_return_none():
        DummyStatement()

    @basic_no_opt.add(dummy_dialect)
    def side_effect_maybe_return_none(cond: bool):
        if cond:
            return
        else:
            DummyStatement()
            return

    @basic_no_opt.add(dummy_dialect)
    def side_effect_intraprocedure(cond: bool):
        if cond:
            return side_effect_return_none()
        else:
            x = (1, 2, 3)
            return x

    @basic_no_opt.add(dummy_dialect)
    def side_effect_true_branch_const(cond: bool):
        if cond:
            return side_effect_maybe_return_none(cond)
        else:
            return cond

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

    result = constprop.eval(
        side_effect_true_branch_const,
        tuple(NotConst() for _ in side_effect_true_branch_const.args),
    ).expect()
    assert isinstance(result, NotConst)  # instead of NotPure
    true_branch = side_effect_true_branch_const.callable_region.blocks[1]
    assert constprop.results[true_branch.stmts.at(1).results[0]] == Const(None)
