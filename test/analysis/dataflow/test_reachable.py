from kirin.analysis.dataflow.reachable import EmptyLattice, ReachableAnalysis
from kirin.prelude import basic_no_opt


@basic_no_opt
def deadblock(x):
    if x:
        return x + 1
    else:
        return x + 2
    return x + 3


def test_reachable():
    dba = ReachableAnalysis(deadblock.dialects)
    dba.eval(deadblock, tuple(EmptyLattice() for _ in deadblock.args))
    assert deadblock.code.body.blocks[-1] not in dba.visited  # type: ignore
