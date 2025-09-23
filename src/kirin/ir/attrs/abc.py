from abc import ABC, ABCMeta, abstractmethod
from typing import TYPE_CHECKING, TypeVar, ClassVar, Optional
from dataclasses import field, dataclass

from kirin.print import Printable
from kirin.ir.traits import Trait
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
    traits: ClassVar[frozenset[Trait["Attribute"]]] = field(
        default=frozenset(), init=False, repr=False
    )
    """Set of Attribute traits."""

    @abstractmethod
    def __hash__(self) -> int: ...

    @abstractmethod
    def __eq__(self, value: object) -> bool: ...

    @classmethod
    def has_trait(cls, trait_type: type[Trait["Attribute"]]) -> bool:
        """Check if the Statement has a specific trait.

        Args:
            trait_type (type[Trait]): The type of trait to check for.

        Returns:
            bool: True if the class has the specified trait, False otherwise.
        """
        for trait in cls.traits:
            if isinstance(trait, trait_type):
                return True
        return False

    TraitType = TypeVar("TraitType", bound=Trait["Attribute"])

    def get_trait(self, trait: type[TraitType]) -> Optional[TraitType]:
        """Get the trait of the attribute.

        Args:
            trait (type[Trait]): the trait to get

        Returns:
            Optional[Trait]: the trait if found, None otherwise
        """
        for t in self.traits:
            if isinstance(t, trait):
                return t

        return None
