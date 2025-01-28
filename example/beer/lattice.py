from typing import Any, final
from dataclasses import field, dataclass

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
class PukePenalty(Item):

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, PukePenalty)


@final
@dataclass
class DrinkFee(Item):
    beer_name: str
    price: float

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, DrinkFee)


@final
@dataclass
class PourFee(Item):
    count: int

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, PourFee)
