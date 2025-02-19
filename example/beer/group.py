from dialect import dialect

from rewrite import RandomWalkBranch
from kirin.ir import dialect_group
from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk, Fixpoint
from kirin.passes.fold import Fold


# create our own food dialect, it runs a random walk on the branches
@dialect_group(basic_no_opt.add(dialect))
def food(self):

    fold_pass = Fold(self)

    def run_pass(mt, *, fold=True):
        Fixpoint(Walk(RandomWalkBranch())).rewrite(mt.code)

        # add const fold
        if fold:
            fold_pass(mt)

    return run_pass
