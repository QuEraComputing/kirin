from dataclasses import dataclass

from kirin.ir.types import TypeAttribute
from kirin.lattice import BoundedLattice

from .const import ConstLattice
from .purity import Purity


@dataclass
class InferenceLattice(BoundedLattice["InferenceLattice"]):
    typ: TypeAttribute
    const: ConstLattice
    purity: Purity

    @classmethod
    def top(cls) -> "InferenceLattice":
        return cls(TypeAttribute.top(), ConstLattice.top(), Purity.top())

    @classmethod
    def bottom(cls) -> "InferenceLattice":
        return cls(TypeAttribute.bottom(), ConstLattice.bottom(), Purity.bottom())

    def is_subseteq(self, other: "InferenceLattice") -> bool:
        return (
            self.typ.is_subseteq(other.typ)
            and self.const.is_subseteq(other.const)
            and self.purity.is_subseteq(other.purity)
        )

    def join(self, other: "InferenceLattice") -> "InferenceLattice":
        return InferenceLattice(
            self.typ.join(other.typ),
            self.const.join(other.const),
            self.purity.join(other.purity),
        )

    def meet(self, other: "InferenceLattice") -> "InferenceLattice":
        return InferenceLattice(
            self.typ.meet(other.typ),
            self.const.meet(other.const),
            self.purity.meet(other.purity),
        )
