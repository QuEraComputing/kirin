"""This module contains some common eDSLs that can be used to build
more complex eDSLs. The eDSLs in this module are built on top of the
basic dialects provided by the `kirin.dialects` module.
"""

from typing_extensions import Doc, Annotated

from kirin.ir import Method, dialect_group
from kirin.passes import aggressive
from kirin.dialects import cf, scf, func, math, ilist, lowering
from kirin.dialects.py import (
    cmp,
    len,
    attr,
    base,
    list,
    binop,
    slice,
    tuple,
    unary,
    assign,
    boolop,
    builtin,
    constant,
    indexing,
    iterable,
    assertion,
)
from kirin.passes.fold import Fold
from kirin.passes.typeinfer import TypeInfer


@dialect_group(
    [
        base,
        binop,
        cmp,
        unary,
        assign,
        attr,
        boolop,
        builtin,
        constant,
        indexing,
        len,
        tuple,
        assertion,
        iterable,
    ]
)
def python_basic(self):
    """The basic Python dialect without list, range, and slice."""

    def run_pass(mt: Method) -> None:
        pass

    return run_pass


@dialect_group(
    python_basic.union(
        [
            list,
            slice,
            cf,
            func,
            lowering.cf,
            lowering.func,
            lowering.call,
            lowering.range.ilist,
            math,
        ]
    )
)
def python_no_opt(self):
    """The Python dialect without optimization passes."""

    def run_pass(mt: Method) -> None:
        pass

    return run_pass


@dialect_group(
    python_basic.union(
        [
            ilist,
            slice,
            cf,
            func,
            math,
            lowering.cf,
            lowering.func,
            lowering.call,
            lowering.range.ilist,
        ]
    )
)
def basic_no_opt(self):
    """The basic kernel without optimization passes. This is a builtin
    eDSL that includes the basic dialects that are commonly used in
    Python-like eDSLs.

    This eDSL includes the basic dialects without any optimization passes.
    Other eDSL can usually be built on top of this eDSL by utilizing the
    `basic_no_opt.add` method to add more dialects and optimization passes.

    Note that unlike Python, list in this eDSL is immutable, and the
    `append` method is not available. Use `+` operator to concatenate lists
    instead. Immutable list is easier to optimize and reason about.

    See also [`basic`][kirin.prelude.basic] for the basic kernel with optimization passes.
    See also [`ilist`][kirin.dialects.ilist] for the immutable list dialect.
    """
    ilist_desugar = ilist.IListDesugar(self)

    def run_pass(mt: Method) -> None:
        ilist_desugar.fixpoint(mt)

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
    ilist_desugar = ilist.IListDesugar(self)
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

        ilist_desugar.fixpoint(mt)

        if fold:
            if aggressive:
                aggressive_fold_pass.fixpoint(mt)
            else:
                fold_pass(mt)

        if typeinfer:
            typeinfer_pass(mt)

    return run_pass


@dialect_group(
    python_basic.union(
        [
            ilist,
            slice,
            scf,
            cf,
            func,
            math,
            lowering.func,
            lowering.call,
            lowering.range.ilist,
        ]
    )
)
def structural_no_opt(self):
    """Structural kernel without optimization passes."""

    def run_pass(method: Method) -> None:
        pass

    return run_pass


@dialect_group(
    python_basic.union(
        [
            ilist,
            slice,
            scf,
            cf,
            func,
            math,
            lowering.func,
            lowering.call,
            lowering.range.ilist,
        ]
    )
)
def structural(self):
    """Structural kernel without optimization passes."""
    typeinfer_pass = TypeInfer(self)

    def run_pass(
        method: Method, *, verify: bool = True, typeinfer: bool = True
    ) -> None:
        if verify:
            method.verify()

        if typeinfer:
            typeinfer_pass(method)

    return run_pass
