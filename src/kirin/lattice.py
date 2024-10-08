from __future__ import annotations

from abc import ABC, ABCMeta, abstractmethod
from typing import Generic, Iterable, TypeVar

LatticeElem = TypeVar("LatticeElem", bound="Lattice", covariant=True)


class LatticeMeta(ABCMeta):
    pass


class Lattice(ABC, Generic[LatticeElem], metaclass=LatticeMeta):

    @abstractmethod
    def join(self, other: Lattice[LatticeElem]) -> Lattice[LatticeElem]: ...

    @abstractmethod
    def meet(self, other: Lattice[LatticeElem]) -> Lattice[LatticeElem]: ...

    def is_bottom(self):
        return self is self.bottom()

    def is_top(self):
        return self is self.top()

    @abstractmethod
    def is_subseteq(self, other: Lattice[LatticeElem]) -> bool: ...

    def is_subset(self, other: Lattice[LatticeElem]) -> bool:
        return self.is_subseteq(other) and not other.is_subseteq(self)

    def __eq__(self, value: object) -> bool:
        return isinstance(value, Lattice) and self.is_equal(value)

    def is_equal(self, other: Lattice[LatticeElem]) -> bool:
        if self is other:
            return True
        else:
            return self.is_subseteq(other) and other.is_subseteq(self)

    @classmethod
    @abstractmethod
    def bottom(cls) -> LatticeElem: ...

    @classmethod
    @abstractmethod
    def top(cls) -> LatticeElem: ...

    @abstractmethod
    def __hash__(self) -> int: ...


class SingletonMeta(LatticeMeta):
    """See https://stackoverflow.com/questions/674304/why-is-init-always-called-after-new/8665179#8665179"""

    def __init__(cls, name, bases, attrs):
        super().__init__(name, bases, attrs)
        cls._instance = None

    def __call__(cls, *args, **kwargs):
        if cls._instance is None:
            cls._instance = super().__call__(*args, **kwargs)
        return cls._instance


class UnionMeta(LatticeMeta):
    """Meta class for union types. It simplifies the union if possible."""

    def __call__(
        self,
        typ: Iterable[Lattice[LatticeElem]] | Lattice[LatticeElem],
        *others: Lattice[LatticeElem],
    ):
        if isinstance(typ, Lattice):
            typs: Iterable[Lattice[LatticeElem]] = (typ, *others)
        elif not others:
            typs = typ
        else:
            raise ValueError(
                "Expected an iterable of types or variadic arguments of types"
            )

        # try if the union can be simplified
        params: list[Lattice[LatticeElem]] = []
        for typ in typs:
            contains = False
            for idx, other in enumerate(params):
                if typ.is_subseteq(other):
                    contains = True
                    break
                elif other.is_subseteq(typ):
                    params[idx] = typ
                    contains = True
                    break

            if not contains:
                params.append(typ)

        if len(params) == 1:
            return params[0]
        return super(UnionMeta, self).__call__(typ, *params)


class EmptyLattice(Lattice["EmptyLattice"], metaclass=SingletonMeta):

    def join(self, other: Lattice[EmptyLattice]) -> Lattice[EmptyLattice]:
        return self

    def meet(self, other: Lattice[EmptyLattice]) -> Lattice[EmptyLattice]:
        return self

    def is_bottom(self):
        return True

    def is_top(self):
        return True

    @classmethod
    def bottom(cls):
        return cls()

    @classmethod
    def top(cls):
        return cls()

    def __hash__(self) -> int:
        return id(self)

    def is_subseteq(self, other: Lattice[EmptyLattice]) -> bool:
        return True
