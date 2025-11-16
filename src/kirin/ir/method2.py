from __future__ import annotations

import typing
from types import ModuleType

from dataclasses import dataclass, field

from kirin.types import TypeAttribute, MethodType
from kirin.ir.nodes.stmt import Statement
from kirin.ir.attrs.types import _MethodType
from kirin.print.printable import Printable

if typing.TYPE_CHECKING:
    from kirin.ir.group import DialectGroup

Param = typing.ParamSpec("Param")
RetType = typing.TypeVar("RetType")


@dataclass
class MethodInfo(typing.Generic[Param, RetType]):
    nargs: int
    """The number of arguments of the method. 0 if no arguments."""
    mod: ModuleType | None = None  # ref
    """The module where the method is defined. None if no module."""
    py_func: typing.Callable[Param, RetType] | None = None  # ref
    """The original Python function. None if no Python function."""
    sym_name: str | None = None
    """The name of the method. None if no name."""
    arg_names: list[str] | None = None
    """The argument names of the callable statement. None if no keyword arguments allowed."""


@dataclass
class StagedMethod(typing.Generic[Param, RetType]):
    """A staged method is a method that belongs to a dialect group and has multiple
    specializations based on their signature.
    """

    dialects: DialectGroup
    """The dialects that creates the method. This should be a DialectGroup."""
    signature: _MethodType
    """The signature of the staged method."""
    specializations: dict[_MethodType, SpecializedMethod[Param, RetType]] = field(
        default_factory=dict
    )
    """The specialized methods of the staged method, keyed by their signature."""


@dataclass
class SpecializedMethod(typing.Generic[Param, RetType]):
    parent: StagedMethod[Param, RetType]
    """The parent staged method."""
    signature: _MethodType
    """The signature of the specialized method, should only contain concrete types not type variables or generics."""
    code: Statement
    """The code of the specialized method."""
    closure_env: tuple = field(default_factory=tuple)
    """values captured in the method if it is a closure."""

    file: str = ""
    """The file where the method is defined. Empty string if no file."""
    lineno_begin: int = 0
    """The line number where the method is defined. 0 if no line number."""
