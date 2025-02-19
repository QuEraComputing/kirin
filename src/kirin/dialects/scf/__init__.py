"""A Python-like structural Control Flow dialect.

This dialect provides constructs for expressing control flow in a structured
manner. The dialect provides constructs for expressing loops and conditionals.
Unlike MLIR SCF dialect, this dialect does not restrict the control flow to
statically analyzable forms. This dialect is designed to be compatible with
Python native control flow constructs.

This dialect depends on the following dialects:
- `eltype`: for obtaining the element type of a value.
"""

from . import (
    trim as trim,
    absint as absint,
    interp as interp,
    unroll as unroll,
    lowering as lowering,
    constprop as constprop,
    typeinfer as typeinfer,
)
from .stmts import For as For, Yield as Yield, IfElse as IfElse
from ._dialect import dialect as dialect
