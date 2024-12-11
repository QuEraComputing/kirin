from typing_extensions import Annotated, Doc

from kirin.dialects import cf, fcf, func, math
from kirin.dialects.py import data, stmts
from kirin.ir import Method, dialect_group
from kirin.passes import aggressive
from kirin.passes.fold import Fold
from kirin.passes.typeinfer import TypeInfer


@dialect_group([cf, fcf, func, math, data, stmts])
def basic_no_opt(self):
    """The basic kernel without optimization passes.

    This eDSL includes the basic dialects without any optimization passes.
    Other eDSL can usually be built on top of this eDSL by utilizing the
    `basic_no_opt.add` method to add more dialects and optimization passes.

    See also [`basic`][kirin.prelude.basic] for the basic kernel with optimization passes.
    """

    def run_pass(mt: Method) -> None:
        pass

    return run_pass


@dialect_group(basic_no_opt)
def basic(self):
    """The basic kernel.

    This eDSL includes the basic dialects and the basic optimization passes.
    Other eDSL can usually be built on top of this eDSL by utilizing the
    `basic.add` method to add more dialects and optimization passes.

    See also [`basic_no_opt`][kirin.prelude.basic_no_opt] for the basic kernel without optimization passes.

    ## Example

    ```python
    from kirin.prelude import basic

    @basic(typeinfer=True)
    def main(x: int) -> int:
        return x + 1 + 1

    main.print() # main is a Method!
    ```
    """
    fold_pass = Fold(self)
    aggressive_fold_pass = aggressive.Fold(self)
    typeinfer_pass = TypeInfer(self)

    def run_pass(
        mt: Annotated[Method, Doc("The method to run pass on.")],
        *,
        verify: Annotated[
            bool, Doc("run `verify` before running passes, default is `True`")
        ] = True,
        typeinfer: Annotated[
            bool,
            Doc(
                "run type inference and apply the inferred type to IR, default `False`"
            ),
        ] = False,
        fold: Annotated[bool, Doc("run folding passes")] = True,
        aggressive: Annotated[
            bool, Doc("run aggressive folding passes if `fold=True`")
        ] = False,
    ) -> None:
        if verify:
            mt.verify()

        if fold:
            if aggressive:
                aggressive_fold_pass.fixpoint(mt)
            else:
                fold_pass(mt)

        if typeinfer:
            typeinfer_pass(mt)

    return run_pass
