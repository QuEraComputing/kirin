from kirin import analysis
from kirin.prelude import basic_no_opt
from kirin.rewrite import Chain, Fixpoint, Walk
from kirin.rules.dce import DeadCodeElimination
from kirin.rules.getitem import InlineGetItem


@basic_no_opt
def main_simplify_getitem(x: int):
    ylist = (x, x, 1, 2)
    return ylist[0]


def test_getitem():
    before = main_simplify_getitem(1)
    constprop = analysis.ConstProp(main_simplify_getitem.dialects)
    constprop.eval(
        main_simplify_getitem,
        tuple(analysis.NotConst() for _ in main_simplify_getitem.args),
    )
    inline_getitem = InlineGetItem(constprop.results)
    Fixpoint(Walk(Chain([inline_getitem, DeadCodeElimination()]))).rewrite(
        main_simplify_getitem.code
    )
    main_simplify_getitem.code.print()
    after = main_simplify_getitem(1)
    assert before == after
    assert len(main_simplify_getitem.callable_region.blocks[0].stmts) == 1
