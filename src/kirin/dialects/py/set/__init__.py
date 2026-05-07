"""The set dialect for Python.

This module contains the dialect for set semantics in Python, including:

- The `New` statement class.
- The lowering pass for set literals and `set()`.
- The concrete implementation of set operations.
"""

from . import interp as interp, lowering as lowering, typeinfer as typeinfer
from .stmts import New as New
from ._dialect import dialect as dialect
