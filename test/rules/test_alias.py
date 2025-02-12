from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk, Chain, Fixpoint, WrapConst
from kirin.analysis import const
from kirin.rewrite.dce import DeadCodeElimination
from kirin.rewrite.alias import InlineAlias


@basic_no_opt
def main_simplify_alias(x: int):
    y = x + 1
    z = y
    z2 = z
    return z2


def test_alias_inline():
    constprop = const.Propagate(main_simplify_alias.dialects)
    frame, ret = constprop.run_analysis(main_simplify_alias)
    Fixpoint(Walk(WrapConst(frame))).rewrite(main_simplify_alias.code)
    Fixpoint(Walk(Chain([InlineAlias(), DeadCodeElimination()]))).rewrite(
        main_simplify_alias.code
    )
    assert len(main_simplify_alias.callable_region.blocks[0].stmts) == 3


@basic_no_opt
def simplify_alias_ref_const():
    y = 3
    z = y
    return z


def test_alias_inline2():
    constprop = const.Propagate(simplify_alias_ref_const.dialects)
    frame, _ = constprop.run_analysis(simplify_alias_ref_const)
    Fixpoint(Walk(WrapConst(frame))).rewrite(main_simplify_alias.code)
    Fixpoint(Walk(Chain([InlineAlias(), DeadCodeElimination()]))).rewrite(
        simplify_alias_ref_const.code
    )
    simplify_alias_ref_const.code.print()
    assert len(simplify_alias_ref_const.callable_region.blocks[0].stmts) == 2
