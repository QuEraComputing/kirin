from abc import abstractmethod
from typing import Generic, TypeVar
from dataclasses import field, dataclass

from .abc import Attribute
from .types import TypeAttribute

T = TypeVar("T", covariant=True)


@dataclass(eq=False)
class Data(Attribute, Generic[T]):
    """Base class for data attributes.

    Data attributes are compile-time constants that can be used to
    represent runtime data inside the IR.

    This class is meant to be subclassed by specific data attributes.
    It provides a `type` attribute that should be set to the type of
    the data.
    """

    type: TypeAttribute = field(init=False, repr=False)

    @abstractmethod
    def unwrap(self) -> T:
        """Returns the underlying data value."""
        ...
