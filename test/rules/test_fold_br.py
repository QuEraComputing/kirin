from kirin.analysis.cfg import CFG
from kirin.analysis.dataflow.constprop import ConstProp, ConstPropBottom
from kirin.prelude import basic_no_opt
from kirin.rewrite import Fixpoint, Walk
from kirin.rules.cfg_compatify import CFGCompactify
from kirin.rules.dce import DeadCodeElimination
from kirin.rules.fold import ConstantFold


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
    const_prop = ConstProp(branch.dialects)
    const_prop.eval(branch, tuple(ConstPropBottom() for _ in branch.args))
    fold = ConstantFold(const_prop.results)
    branch.code.print()
    Fixpoint(Walk(fold)).rewrite(branch.code)
    branch.code.print()
    cfg = CFG(branch.callable_region)
    # TODO: also check the generated CFG
    # interp.worklist.visited
    Fixpoint(CFGCompactify(cfg)).rewrite(branch.code)
    Walk(DeadCodeElimination()).rewrite(branch.code)
    branch.code.print()
    assert len(branch.code.body.blocks) == 4  # type: ignore
