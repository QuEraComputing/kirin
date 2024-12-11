from dataclasses import dataclass
from typing import Any, final

from kirin import ir
from kirin.lattice import (
    BoundedLattice,
    IsSubsetEqMixin,
    LatticeMeta,
    SimpleJoinMixin,
    SimpleMeetMixin,
    SingletonMeta,
)

from ._const import _ConstLattice


@dataclass
class ConstLattice(
    IsSubsetEqMixin["ConstLattice"],
    SimpleJoinMixin["ConstLattice"],
    SimpleMeetMixin["ConstLattice"],
    BoundedLattice["ConstLattice"],
    _ConstLattice,
):

    @classmethod
    def top(cls) -> "ConstLattice":
        return NotConst()

    @classmethod
    def bottom(cls) -> "ConstLattice":
        return Unknown()


@final
@dataclass
class NotConst(ConstLattice, metaclass=SingletonMeta):

    def is_subseteq(self, other: ConstLattice) -> bool:
        return isinstance(other, NotConst)


@final
@dataclass
class Unknown(ConstLattice, metaclass=SingletonMeta):

    def is_subseteq(self, other: ConstLattice) -> bool:
        return True


@final
@dataclass
class Const(ConstLattice):
    data: Any

    def is_subseteq_Const(self, other: "Const") -> bool:
        return self.data == other.data

    def is_equal(self, other: ConstLattice) -> bool:
        if isinstance(other, Const):
            return self.data == other.data
        return False


@final
class PartialTupleMeta(LatticeMeta):
    def __call__(cls, data: tuple[ConstLattice, ...]):
        if all(isinstance(x, Const) for x in data):
            return Const(tuple(x.data for x in data))  # type: ignore
        return super().__call__(data)


@final
@dataclass
class PartialTuple(ConstLattice, metaclass=PartialTupleMeta):
    data: tuple[ConstLattice, ...]

    def join(self, other: ConstLattice) -> ConstLattice:
        if other.is_subseteq(self):
            return self
        elif self.is_subseteq(other):
            return other
        elif isinstance(other, PartialTuple):
            return PartialTuple(tuple(x.join(y) for x, y in zip(self.data, other.data)))
        elif isinstance(other, Const) and isinstance(other.data, tuple):
            return PartialTuple(
                tuple(x.join(Const(y)) for x, y in zip(self.data, other.data))
            )
        return NotConst()

    def meet(self, other: ConstLattice) -> ConstLattice:
        if self.is_subseteq(other):
            return self
        elif other.is_subseteq(self):
            return other
        elif isinstance(other, PartialTuple):
            return PartialTuple(tuple(x.meet(y) for x, y in zip(self.data, other.data)))
        elif isinstance(other, Const) and isinstance(other.data, tuple):
            return PartialTuple(
                tuple(x.meet(Const(y)) for x, y in zip(self.data, other.data))
            )
        return self.bottom()

    def is_equal(self, other: ConstLattice) -> bool:
        if isinstance(other, PartialTuple):
            return all(x.is_equal(y) for x, y in zip(self.data, other.data))
        elif isinstance(other, Const) and isinstance(other.data, tuple):
            return all(x.is_equal(Const(y)) for x, y in zip(self.data, other.data))
        return False

    def is_subseteq_PartialTuple(self, other: "PartialTuple") -> bool:
        return all(x.is_subseteq(y) for x, y in zip(self.data, other.data))

    def is_subseteq_Const(self, other: Const) -> bool:
        if isinstance(other.data, tuple):
            return all(x.is_subseteq(Const(y)) for x, y in zip(self.data, other.data))
        return False


@final
@dataclass
class PartialLambda(ConstLattice):
    argnames: list[str]
    code: ir.Statement
    captured: tuple[ConstLattice, ...]

    def is_subseteq_PartialLambda(self, other: "PartialLambda") -> bool:
        if self.code is not other.code:
            return False
        if len(self.captured) != len(other.captured):
            return False

        return all(x.is_subseteq(y) for x, y in zip(self.captured, other.captured))

    def join(self, other: ConstLattice) -> ConstLattice:
        if other is other.bottom():
            return self

        if not isinstance(other, PartialLambda):
            return NotConst().join(other)  # widen self

        if self.code is not other.code:
            return NotConst()  # lambda stmt is pure

        if len(self.captured) != len(other.captured):
            return self.bottom()  # err

        return PartialLambda(
            self.argnames,
            self.code,
            tuple(x.join(y) for x, y in zip(self.captured, other.captured)),
        )

    def meet(self, other: ConstLattice) -> ConstLattice:
        if not isinstance(other, PartialLambda):
            return NotConst().meet(other)

        if self.code is not other.code:
            return self.bottom()

        if len(self.captured) != len(other.captured):
            return NotConst()

        return PartialLambda(
            self.argnames,
            self.code,
            tuple(x.meet(y) for x, y in zip(self.captured, other.captured)),
        )
