from dialect import _dialect

from kirin import ir
from kirin.prelude import basic_no_opt


@ir._dialectgroup(basic_no_opt.add(dialect=_dialect))
def pauli(self):
    def run_pass(mt):
        # TODO
        pass

    return run_pass
