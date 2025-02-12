from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk, Chain, Fixpoint, WrapConst
from kirin.analysis import const
from kirin.rewrite.dce import DeadCodeElimination
from kirin.rewrite.getitem import InlineGetItem


@basic_no_opt
def main_simplify_getitem(x: int):
    ylist = (x, x, 1, 2)
    return ylist[0]


def test_getitem():
    before = main_simplify_getitem(1)
    constprop = const.Propagate(main_simplify_getitem.dialects)
    frame, _ = constprop.run_analysis(main_simplify_getitem)
    Fixpoint(Walk(WrapConst(frame))).rewrite(main_simplify_getitem.code)
    inline_getitem = InlineGetItem()
    Fixpoint(Walk(Chain([inline_getitem, DeadCodeElimination()]))).rewrite(
        main_simplify_getitem.code
    )
    main_simplify_getitem.code.print()
    after = main_simplify_getitem(1)
    assert before == after
    assert len(main_simplify_getitem.callable_region.blocks[0].stmts) == 1
