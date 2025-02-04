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
class PourFeeItem(Item):
    count: int

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, PourFeeItem)


@final
@dataclass
class AtLeastItem(Item):
    lower_bound: int

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, AtLeastItem) and self.lower_bound == other.lower_bound


@final
@dataclass
class ConstIntItem(Item):
    lower_bound: int

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, AtLeastItem) and self.lower_bound == other.lower_bound
