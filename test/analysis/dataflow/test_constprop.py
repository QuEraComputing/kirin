from kirin import ir
from kirin.analysis.dataflow.constprop import (
    AnyConst,
    Const,
    ConstProp,
    NotConst,
    PartialTuple,
)
from kirin.prelude import basic_no_opt


class TestLattice:

    def test_meet(self):
        assert AnyConst().meet(AnyConst()) == AnyConst()
        assert AnyConst().meet(NotConst()) == NotConst()
        assert AnyConst().meet(Const(1)) == Const(1)
        assert NotConst().meet(AnyConst()) == NotConst()
        assert NotConst().meet(NotConst()) == NotConst()
        assert NotConst().meet(Const(1)) == NotConst()
        assert Const(1).meet(AnyConst()) == Const(1)
        assert Const(1).meet(NotConst()) == NotConst()
        assert Const(1).meet(Const(1)) == Const(1)

    def test_join(self):
        assert AnyConst().join(AnyConst()) == AnyConst()
        assert AnyConst().join(NotConst()) == AnyConst()
        assert AnyConst().join(Const(1)) == AnyConst()
        assert NotConst().join(AnyConst()) == AnyConst()
        assert NotConst().join(NotConst()) == NotConst()
        assert NotConst().join(Const(1)) == Const(1)
        assert Const(1).join(AnyConst()) == AnyConst()
        assert Const(1).join(NotConst()) == Const(1)
        assert Const(1).join(Const(1)) == Const(1)
        assert Const(1).join(Const(2)) == AnyConst()

    def test_is_equal(self):
        assert AnyConst().is_equal(AnyConst())
        assert not AnyConst().is_equal(NotConst())
        assert not AnyConst().is_equal(Const(1))
        assert NotConst().is_equal(NotConst())
        assert not NotConst().is_equal(Const(1))
        assert Const(1).is_equal(Const(1))
        assert not Const(1).is_equal(Const(2))

    def test_partial_tuple(self):
        pt1 = PartialTuple((Const(1), NotConst()))
        pt2 = PartialTuple((Const(1), NotConst()))
        assert pt1.is_equal(pt2)
        assert pt1.is_subseteq(pt2)
        assert pt1.join(pt2) == pt1
        assert pt1.meet(pt2) == pt1
        pt2 = PartialTuple((Const(1), Const(2)))
        assert not pt1.is_equal(pt2)
        assert pt1.is_subseteq(pt2)
        assert pt1.join(pt2) == PartialTuple((Const(1), Const(2)))
        assert pt1.meet(pt2) == PartialTuple((Const(1), NotConst()))
        pt2 = PartialTuple((Const(1), NotConst()))
        assert pt1.is_equal(pt2)
        assert pt1.is_subseteq(pt2)
        assert pt1.join(pt2) == pt1
        assert pt1.meet(pt2) == pt1
        pt2 = PartialTuple((Const(1), AnyConst()))
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
        == AnyConst()
    )
    assert infer.eval(
        recurse, tuple(NotConst() for _ in recurse.args)
    ).expect() == Const((0, 0, 0))
