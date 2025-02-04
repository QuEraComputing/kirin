"""Python dialects module.

This module contains a set of dialects that represent
different fractions of the Python language. The dialects
are designed to be used in a union to represent the
entire Python language.
"""

from . import (
    cmp as cmp,
    len as len,
    attr as attr,
    base as base,
    list as list,
    binop as binop,
    range as range,
    slice as slice,
    tuple as tuple,
    unary as unary,
    assign as assign,
    boolop as boolop,
    unpack as unpack,
    builtin as builtin,
    constant as constant,
    indexing as indexing,
    iterable as iterable,
)
from .len import Len as Len
from .attr import GetAttr as GetAttr
from .range import Range as Range
from .slice import Slice as Slice
from .assign import Alias as Alias, SetItem as SetItem
from .boolop import Or as Or, And as And
from .builtin import Abs as Abs, Sum as Sum
from .constant import Constant as Constant
from .indexing import GetItem as GetItem, PyGetItemLike as PyGetItemLike
from .cmp.stmts import *  # noqa: F403
from .list.stmts import Append as Append
from .binop.stmts import *  # noqa: F403
from .unary.stmts import *  # noqa: F403
