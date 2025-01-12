"""
Immutable list dialect for Python.

This dialect provides a simple, immutable list dialect similar
to Python's built-in list type.
"""

from . import interp as interp, lowering as lowering, typeinfer as typeinfer
from .stmts import (
    Map as Map,
    New as New,
    Scan as Scan,
    FoldL as FoldL,
    FoldR as FoldR,
    ForEach as ForEach,
)
from ._dialect import dialect as dialect
