# re-exports the public API of the kirin package
from kirin import ir

from . import types as types
from .exception import enable_stracetrace, disable_stracetrace

__all__ = ["ir", "types", "enable_stracetrace", "disable_stracetrace"]
