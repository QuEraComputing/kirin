from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk, Fixpoint
from kirin.analysis import const
from kirin.rewrite.dce import DeadCodeElimination
from kirin.analysis.cfg import CFG
from kirin.rewrite.fold import ConstantFold
from kirin.rewrite.cfg_compactify import CFGCompactify


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
    results, ret = const_prop.run_analysis(branch)
    fold = ConstantFold(results)
    branch.code.print()
    Fixpoint(Walk(fold)).rewrite(branch.code)
    branch.code.print()
    cfg = CFG(branch.callable_region)
    # TODO: also check the generated CFG
    # interp.worklist.visited
    Fixpoint(CFGCompactify(cfg)).rewrite(branch.code)
    Walk(DeadCodeElimination(results)).rewrite(branch.code)
    branch.code.print()
    assert len(branch.code.body.blocks) == 4  # type: ignore
