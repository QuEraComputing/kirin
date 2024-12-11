from dataclasses import dataclass
from typing import Generic, TypeVarTuple

from kirin.lattice import Lattice

LatticeTypes = TypeVarTuple("LatticeTypes")


@dataclass
class Product(Lattice["Product"], Generic[*LatticeTypes]):
    """Product lattice."""

    lattices: tuple[*LatticeTypes]

    @classmethod
    def top(cls) -> "Product[*LatticeTypes]":
        return Top()

    @classmethod
    def bottom(cls) -> "Product[*LatticeTypes]":
        return Bottom()


class Top(Product[*LatticeTypes]):

    def join(self, other: Product) -> Product:
        return self

    def meet(self, other: Product) -> Product:
        return other

    def is_subseteq(self, other: Product) -> bool:
        return True

    def is_equal(self, other: Product) -> bool:
        return other is self

    def __hash__(self) -> int:
        return id(self)


class Bottom(Product[*LatticeTypes]):

    def join(self, other: Product) -> Product:
        return other

    def meet(self, other: Product) -> Product:
        return self

    def is_subseteq(self, other: Product) -> bool:
        return True

    def is_equal(self, other: Product) -> bool:
        return other is self

    def __hash__(self) -> int:
        return id(self)
