from kirin import ir
from kirin.prelude import basic_no_opt

from dialect import dialect_


@ir.dialect_group(basic_no_opt.add(dialect=dialect_))
def pauli(self):
    def run_pass(mt):
        # TODO
        pass

    return run_pass
