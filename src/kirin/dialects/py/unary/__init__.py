"""The unary dialect for Python.

This module contains the dialect for unary semantics in Python, including:

- The `UnaryOp` base class for unary operations.
- The `UAdd`, `USub`, `Not`, and `Invert` statement classes.
- The lowering pass for unary operations.
- The concrete implementation of unary operations.
- The type inference implementation of unary operations.
- The constant propagation implementation of unary operations.
- The Julia emitter for unary operations.

This dialect maps `ast.UnaryOp` nodes to the `UAdd`, `USub`, `Not`, and `Invert` statements.
"""

from . import (
    julia as julia,
    interp as interp,
    lowering as lowering,
    constprop as constprop,
    typeinfer as typeinfer,
)
from .stmts import *  # noqa: F403
from ._dialect import dialect as dialect
