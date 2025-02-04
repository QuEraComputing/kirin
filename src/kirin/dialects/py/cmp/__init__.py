"""The cmp dialect for Python.

This module contains the dialect for comparison semantics in Python, including:

- The `Eq`, `NotEq`, `Lt`, `LtE`, `Gt`, `GtE`, `Is`, and `IsNot` statement classes.
- The lowering pass for comparison operations.
- The concrete implementation of comparison operations.
- The Julia emitter for comparison operations.

This dialect maps `ast.Compare` nodes to the `Eq`, `NotEq`, `Lt`, `LtE`,
`Gt`, `GtE`, `Is`, and `IsNot` statements.
"""

from . import julia as julia, interp as interp, lowering as lowering
from .stmts import *  # noqa: F403
from ._dialect import dialect as dialect
