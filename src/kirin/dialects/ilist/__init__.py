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
from ._wrapper import (  # careful this is not the builtin range
    map as map,
    scan as scan,
    foldl as foldl,
    foldr as foldr,
    range as range,
    for_each as for_each,
)
