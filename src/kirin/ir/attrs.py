from __future__ import annotations

from abc import ABC, ABCMeta, abstractmethod
from typing import TYPE_CHECKING, ClassVar
from dataclasses import field, dataclass

from kirin.print import Printable

if TYPE_CHECKING:
    from kirin.ir.dialect import Dialect


class AttributeMeta(ABCMeta):
    """Metaclass for attributes."""

    pass


@dataclass(eq=False)
class Attribute(ABC, Printable, metaclass=AttributeMeta):
    dialect: ClassVar[Dialect | None] = field(default=None, init=False, repr=False)
    name: ClassVar[str] = field(init=False, repr=False)

    @abstractmethod
    def __hash__(self) -> int: ...
