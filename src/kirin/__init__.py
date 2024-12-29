# re-exports the public API of the kirin package
from kirin import ir
from kirin.ir import types as types
from kirin.decl import info, statement

__all__ = ["ir", "statement", "info"]
