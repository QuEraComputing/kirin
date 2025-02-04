"""The list dialect for Python.

This module contains the dialect for list semantics in Python, including:

- The `New` and `Append` statement classes.
- The lowering pass for list operations.
- The concrete implementation of list operations.
- The type inference implementation of list operations.

This dialect maps `list()`, `ast.List` and `append()` calls to the `New` and `Append` statements.
"""

from . import interp as interp, lowering as lowering, typeinfer as typeinfer
from .stmts import New as New, Append as Append
from ._dialect import dialect as dialect
