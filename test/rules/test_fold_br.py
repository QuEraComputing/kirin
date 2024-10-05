from kirin.analysis.dataflow.reachable import ReachableAnalysis
from kirin.interp import Interpreter
from kirin.prelude import basic_no_opt
from kirin.rewrite import Fixpoint, Walk
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
    fold = ConstantFold(Interpreter(branch.dialects))
    branch.code.print()
    Fixpoint(Walk(fold)).rewrite(branch.code)
    branch.code.print()
    interp = ReachableAnalysis(branch.dialects)
    interp.run_analysis(branch)
    # TODO: also check the generated CFG
    # interp.worklist.visited
    Walk(DeadCodeElimination(interp.worklist.visited)).rewrite(branch.code)
    branch.code.print()
    assert len(branch.code.body.blocks) == 4
