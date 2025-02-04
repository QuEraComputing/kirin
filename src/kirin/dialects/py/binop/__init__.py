"""The binop dialect for Python.

This module contains the dialect for binary operation semantics in Python, including:

- The `Add`, `Sub`, `Mult`, `Div`, `FloorDiv`, `Mod`, `Pow`,
    `LShift`, `RShift`, `BitOr`, `BitXor`, and `BitAnd` statement classes.
- The lowering pass for binary operations.
- The concrete implementation of binary operations.
- The type inference implementation of binary operations.
- The Julia emitter for binary operations.

This dialect maps `ast.BinOp` nodes to the `Add`, `Sub`, `Mult`, `Div`, `FloorDiv`,
`Mod`, `Pow`, `LShift`, `RShift`, `BitOr`, `BitXor`, and `BitAnd` statements.
"""

from . import (
    julia as julia,
    interp as interp,
    lowering as lowering,
    typeinfer as typeinfer,
)
from .stmts import *  # noqa: F403
from ._dialect import dialect as dialect
