from abc import ABC, ABCMeta, abstractmethod
from typing import TYPE_CHECKING, ClassVar, Optional
from dataclasses import field, dataclass

from kirin.print import Printable

if TYPE_CHECKING:
    from kirin.ir.dialect import Dialect


class AttributeMeta(ABCMeta):
    """Metaclass for attributes."""

    pass


@dataclass(eq=False)
class Attribute(ABC, Printable, metaclass=AttributeMeta):
    """ABC for compile-time values."""

    dialect: ClassVar[Optional["Dialect"]] = field(default=None, init=False, repr=False)
    """Dialect of the attribute. (default: None)"""
    name: ClassVar[str] = field(init=False, repr=False)
    """Name of the attribute in printing and other text format."""

    @abstractmethod
    def __hash__(self) -> int: ...
