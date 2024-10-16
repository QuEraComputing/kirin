from kirin.dialects import cf, fcf, func, math
from kirin.dialects.py import data, stmts, types
from kirin.ir import Method, dialect_group
from kirin.passes.fold import Fold
from kirin.passes.typeinfer import TypeInfer


@dialect_group([cf, fcf, func, math, types, data, stmts])
def basic_no_opt(self):
    def run_pass(mt: Method) -> None:
        pass

    return run_pass


@dialect_group(basic_no_opt)
def basic(self):
    fold_pass = Fold(self)
    typeinfer_pass = TypeInfer(self)

    def run_pass(
        mt: Method, *, verify: bool = True, typeinfer: bool = False, fold: bool = True
    ) -> None:
        if verify:
            mt.verify()

        if fold:
            fold_pass(mt)

        if typeinfer:
            typeinfer_pass(mt)

    return run_pass
