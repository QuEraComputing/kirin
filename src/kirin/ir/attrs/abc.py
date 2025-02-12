from abc import ABC, ABCMeta, abstractmethod
from typing import TYPE_CHECKING, ClassVar, Optional
from dataclasses import field, dataclass

from kirin.print import Printable
from kirin.lattice.abc import LatticeMeta, SingletonMeta

if TYPE_CHECKING:
    from kirin.ir.dialect import Dialect


class AttributeMeta(ABCMeta):
    """Metaclass for attributes."""

    pass


class LatticeAttributeMeta(LatticeMeta, AttributeMeta):
    """Metaclass for lattice attributes."""

    pass


class SingletonLatticeAttributeMeta(LatticeAttributeMeta, SingletonMeta):
    """Metaclass for singleton lattice attributes."""

    pass


@dataclass(eq=False)
class Attribute(ABC, Printable, metaclass=AttributeMeta):
    """ABC for compile-time values. All attributes are hashable
    and thus need to implement the `__hash__` method.

    !!! note "Pretty Printing"
        This object is pretty printable via
        [`.print()`][kirin.print.printable.Printable.print] method.
    """

    dialect: ClassVar[Optional["Dialect"]] = field(default=None, init=False, repr=False)
    """Dialect of the attribute. (default: None)"""
    name: ClassVar[str] = field(init=False, repr=False)
    """Name of the attribute in printing and other text format."""

    @abstractmethod
    def __hash__(self) -> int: ...
