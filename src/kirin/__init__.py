# re-exports the public API of the kirin package
from kirin import ir

from . import types as types

__all__ = ["ir", "types"]
