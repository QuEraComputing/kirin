# re-exports the public API of the kirin package
from kirin import ir
from kirin.decl import info, statement
from kirin.ir import types as types

__all__ = ["ir", "statement", "info"]
