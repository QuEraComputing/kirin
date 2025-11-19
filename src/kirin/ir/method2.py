from __future__ import annotations

import typing
from types import ModuleType

from dataclasses import dataclass, field

from kirin.types import TypeAttribute, MethodType
from kirin.ir.nodes.stmt import Statement
from kirin.ir.attrs.types import FunctionType
from kirin.print.printable import Printable

if typing.TYPE_CHECKING:
    from kirin.ir.group import DialectGroup

Param = typing.ParamSpec("Param")
RetType = typing.TypeVar("RetType")


@dataclass
class Method(typing.Generic[Param, RetType]):
    dialects: DialectGroup
    """The dialects that creates the method. This should be a DialectGroup."""
    signature: FunctionType
    """The signature of the method."""
    staged_method: dict[DialectGroup, StagedMethod[Param, RetType]]
    """The staged methods of the method, keyed by their dialect group."""
    backedges: set[Method] = field(init=False, repr=False)
    """Cache for the backedges. (who calls this method)"""

    py_module: ModuleType | None = None  # ref
    """The module where the method is defined. None if no module."""
    py_func: typing.Callable[Param, RetType] | None = None  # ref
    """The original Python function. None if no Python function."""
    sym_name: str | None = None
    """The name of the method. None if no name."""
    arg_names: list[str] | None = None
    """The argument names of the callable statement. None if no keyword arguments allowed."""
    closure_env: tuple = field(default_factory=tuple)
    """values captured in the method if it is a closure."""

    def __init__(
        self,
        dialects: DialectGroup,
        code: Statement,
        *,
        py_module: ModuleType | None = None,
        py_func: typing.Callable[Param, RetType] | None = None,
        sym_name: str | None = None,
        arg_names: list[str] | None = None,
        closure_env: tuple = (),
        file: str = "",
        lineno_begin: int = 0,
    ):
        self.dialects = dialects
        self.staged_method = {}
        self.py_module = py_module
        self.py_func = py_func
        self.sym_name = sym_name
        self.arg_names = arg_names
        self.closure_env = closure_env
        self.file = file
        self.lineno_begin = lineno_begin


@dataclass
class StagedMethod(typing.Generic[Param, RetType]):
    """A staged method is a method that belongs to a dialect group and has multiple
    specializations based on their signature.
    """

    parent: Method[Param, RetType]
    """The parent method."""
    dialects: DialectGroup
    """The dialects that creates the method. This should be a DialectGroup."""
    signature: FunctionType
    """The signature of the staged method."""
    code: Statement
    """The code of the staged method (unspecialized)."""
    specializations: dict[FunctionType, SpecializedMethod[Param, RetType]] = field(
        default_factory=dict
    )
    """The specialized methods of the staged method, keyed by their signature."""
    backedges: set[StagedMethod] = field(init=False, repr=False)
    """Cache for the backedges. (who calls this method)"""


@dataclass
class SpecializedMethod(typing.Generic[Param, RetType]):
    parent: StagedMethod[Param, RetType]
    """The parent staged method."""
    signature: FunctionType
    """The signature of the specialized method, should only contain concrete types not type variables or generics."""
    code: Statement
    """The code of the specialized method."""

    file: str = ""
    """The file where the method is defined. Empty string if no file."""
    lineno_begin: int = 0
    """The line number where the method is defined. 0 if no line number."""
    backedges: set[SpecializedMethod] = field(init=False, repr=False)
    """Cache for the backedges. (who calls this method)"""
