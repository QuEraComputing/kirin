from kirin.analysis import const
from kirin.prelude import basic_no_opt
from kirin.rewrite import Chain, Fixpoint, Walk
from kirin.rules.alias import InlineAlias
from kirin.rules.dce import DeadCodeElimination


@basic_no_opt
def main_simplify_alias(x: int):
    y = x + 1
    z = y
    z2 = z
    return z2


def test_alias_inline():
    constprop = const.Propagate(main_simplify_alias.dialects)
    constprop.eval(
        main_simplify_alias,
        tuple(const.JointResult.top() for _ in main_simplify_alias.args),
    )
    Fixpoint(
        Walk(Chain([InlineAlias(), DeadCodeElimination(constprop.results)]))
    ).rewrite(main_simplify_alias.code)
    assert len(main_simplify_alias.callable_region.blocks[0].stmts) == 3


@basic_no_opt
def simplify_alias_ref_const():
    y = 3
    z = y
    return z


def test_alias_inline2():
    constprop = const.Propagate(simplify_alias_ref_const.dialects)
    constprop.eval(
        simplify_alias_ref_const,
        tuple(const.JointResult.top() for _ in simplify_alias_ref_const.args),
    )
    Fixpoint(
        Walk(Chain([InlineAlias(), DeadCodeElimination(constprop.results)]))
    ).rewrite(simplify_alias_ref_const.code)
    simplify_alias_ref_const.code.print()
    assert len(simplify_alias_ref_const.callable_region.blocks[0].stmts) == 2
