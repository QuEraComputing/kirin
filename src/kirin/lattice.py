from __future__ import annotations

from abc import ABC, ABCMeta, abstractmethod
from typing import Generic, Iterable, TypeVar

LatticeParent = TypeVar("LatticeParent", bound="Lattice")


class LatticeMeta(ABCMeta):
    """Metaclass for lattices."""

    pass


class Lattice(ABC, Generic[LatticeParent], metaclass=LatticeMeta):
    """Abstract base class for lattices."""

    @abstractmethod
    def join(self, other: LatticeParent) -> LatticeParent: ...

    @abstractmethod
    def meet(self, other: LatticeParent) -> LatticeParent: ...

    @abstractmethod
    def is_subseteq(self, other: LatticeParent) -> bool: ...

    def is_subset(self, other: LatticeParent) -> bool:
        return self.is_subseteq(other) and not other.is_subseteq(self)

    def __eq__(self, value: object) -> bool:
        raise NotImplementedError(
            "Equality is not implemented for lattices, use is_equal instead"
        )

    def is_equal(self, other: LatticeParent) -> bool:
        """Check if two lattices are equal."""
        if self is other:
            return True
        else:
            return self.is_subseteq(other) and other.is_subseteq(self)

    @classmethod
    @abstractmethod
    def bottom(cls) -> LatticeParent: ...

    @classmethod
    @abstractmethod
    def top(cls) -> LatticeParent: ...

    def __hash__(self) -> int:
        raise NotImplementedError("Hash is not implemented for lattices")


class SingletonMeta(LatticeMeta):
    """
    Singleton metaclass for lattices. It ensures that only one instance of a lattice is created.

    See https://stackoverflow.com/questions/674304/why-is-init-always-called-after-new/8665179#8665179
    """

    def __init__(cls, name, bases, attrs):
        super().__init__(name, bases, attrs)
        cls._instance = None

    def __call__(cls):
        if cls._instance is None:
            cls._instance = super().__call__()
        return cls._instance


class UnionMeta(LatticeMeta):
    """Meta class for union types. It simplifies the union if possible."""

    def __call__(
        self,
        typ: Iterable[LatticeParent] | LatticeParent,
        *others: LatticeParent,
    ):
        if isinstance(typ, Lattice):
            typs: Iterable[LatticeParent] = (typ, *others)
        elif not others:
            typs = typ
        else:
            raise ValueError(
                "Expected an iterable of types or variadic arguments of types"
            )

        # try if the union can be simplified
        params: list[LatticeParent] = []
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

        return super(UnionMeta, self).__call__(*params)


class EmptyLattice(Lattice["EmptyLattice"], metaclass=SingletonMeta):
    """Empty lattice."""

    def join(self, other: EmptyLattice) -> EmptyLattice:
        return self

    def meet(self, other: EmptyLattice) -> EmptyLattice:
        return self

    @classmethod
    def bottom(cls):
        return cls()

    @classmethod
    def top(cls):
        return cls()

    def __hash__(self) -> int:
        return id(self)

    def is_subseteq(self, other: EmptyLattice) -> bool:
        return True
