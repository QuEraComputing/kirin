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

    def is_subseteq(self, other: Item) -> bool:
        return True


@final
@dataclass
class AnyItem(Item, metaclass=SingletonMeta):

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, AnyItem)


@final
@dataclass
class PintsItem(Item):
    count: Item
    brand: str

    def is_subseteq(self, other: Item) -> bool:
        return (
            isinstance(other, PintsItem)
            and self.count == other.count
            and self.brand == other.brand
        )


@final
@dataclass
class AtLeastItem(Item):
    data: int

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, AtLeastItem) and self.data == other.data


@final
@dataclass
class ConstIntItem(Item):
    data: int

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, ConstIntItem) and self.data == other.data


@final
@dataclass
class BeerItem(Item):
    brand: str

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, BeerItem) and self.brand == other.brand
