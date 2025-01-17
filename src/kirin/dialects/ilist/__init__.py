"""
Immutable list dialect for Python.

This dialect provides a simple, immutable list dialect similar
to Python's built-in list type.
"""

from . import (
    interp as interp,
    rewrite as rewrite,
    lowering as lowering,
    typeinfer as typeinfer,
)
from .stmts import (
    Map as Map,
    New as New,
    Push as Push,
    Scan as Scan,
    Foldl as Foldl,
    Foldr as Foldr,
    ForEach as ForEach,
    IListType as IListType,
)
from .passes import IListDesugar as IListDesugar
from .runtime import IList as IList
from ._dialect import dialect as dialect
