from typing import final
from dataclasses import dataclass

from kirin.lattice import (
    SingletonMeta,
    BoundedLattice,
    IsSubsetEqMixin,
    SimpleJoinMixin,
    SimpleMeetMixin,
)


@dataclass
class Item(
    IsSubsetEqMixin["Item"],
    SimpleJoinMixin["Item"],
    SimpleMeetMixin["Item"],
    BoundedLattice["Item"],
):

    @classmethod
    def top(cls) -> "Item":
        return AnyItem()

    @classmethod
    def bottom(cls) -> "Item":
        return NotItem()


@final
@dataclass
class NotItem(Item, metaclass=SingletonMeta):
    """The bottom of the lattice.

    Since the element is the same without any field,
    we can use the SingletonMeta to make it a singleton by inherit the metaclass

    """

    def is_subseteq(self, other: Item) -> bool:
        return True


@final
@dataclass
class AnyItem(Item, metaclass=SingletonMeta):
    """The top of the lattice.

    Since the element is the same without any field,
    we can use the SingletonMeta to make it a singleton by inherit the metaclass

    """

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, AnyItem)


@final
@dataclass
class ItemServing(Item):
    count: Item
    type: str

    def is_subseteq(self, other: Item) -> bool:
        return (
            isinstance(other, ItemServing)
            and self.count == other.count
            and self.type == other.type
        )


@final
@dataclass
class AtLeastXItem(Item):
    data: int

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, AtLeastXItem) and self.data == other.data


@final
@dataclass
class ConstIntItem(Item):
    data: int

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, ConstIntItem) and self.data == other.data


@final
@dataclass
class ItemFood(Item):
    type: str

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, ItemFood) and self.type == other.type
