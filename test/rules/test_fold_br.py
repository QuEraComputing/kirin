from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk, Fixpoint, WrapConst
from kirin.analysis import const
from kirin.rewrite.dce import DeadCodeElimination
from kirin.rewrite.fold import ConstantFold
from kirin.rewrite.compactify import CFGCompactify


@basic_no_opt
def branch(x):
    if x > 1:
        y = x + 1
    else:
        y = x + 2

    if True:
        return y + 1
    else:
        y + 2


def test_branch_elim():
    assert branch(1) == 4
    const_prop = const.Propagate(branch.dialects)
    frame, ret = const_prop.run_analysis(branch)
    Walk(Fixpoint(WrapConst(frame))).rewrite(branch.code)
    fold = ConstantFold()
    Fixpoint(Walk(fold)).rewrite(branch.code)
    # TODO: also check the generated CFG
    # interp.worklist.visited
    Fixpoint(CFGCompactify()).rewrite(branch.code)
    Walk(DeadCodeElimination()).rewrite(branch.code)
    branch.code.print()
    assert len(branch.code.body.blocks) == 4  # type: ignore
