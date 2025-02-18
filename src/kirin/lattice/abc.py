from abc import ABC, ABCMeta, abstractmethod
from typing import Generic, TypeVar, Iterable


class LatticeMeta(ABCMeta):
    pass


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


LatticeType = TypeVar("LatticeType", bound="Lattice")


class Lattice(ABC, Generic[LatticeType], metaclass=LatticeMeta):
    """ABC for lattices as Python class.

    While `Lattice` is only an interface, `LatticeABC` is an abstract
    class that can be inherited from. This provides a few default
    implementations for the lattice operations.
    """

    @abstractmethod
    def join(self, other: LatticeType) -> LatticeType:
        """Join operation."""
        ...

    @abstractmethod
    def meet(self, other: LatticeType) -> LatticeType:
        """Meet operation."""
        ...

    @abstractmethod
    def is_subseteq(self, other: LatticeType) -> bool:
        """Subseteq operation."""
        ...

    def is_equal(self, other: LatticeType) -> bool:
        """Check if two lattices are equal."""
        if self is other:
            return True
        else:
            return self.is_subseteq(other) and other.is_subseteq(self)

    def is_subset(self, other: LatticeType) -> bool:
        return self.is_subseteq(other) and not other.is_subseteq(self)

    def __eq__(self, value: object) -> bool:
        raise NotImplementedError(
            "Equality is not implemented for lattices, use is_equal instead"
        )

    def __hash__(self) -> int:
        raise NotImplementedError("Hash is not implemented for lattices")


BoundedLatticeType = TypeVar("BoundedLatticeType", bound="BoundedLattice")


class BoundedLattice(Lattice[BoundedLatticeType]):
    """ABC for bounded lattices as Python class.

    `BoundedLattice` is an abstract class that can be inherited from.
    It requires the implementation of the `bottom` and `top` methods.
    """

    @classmethod
    @abstractmethod
    def bottom(cls) -> BoundedLatticeType: ...

    @classmethod
    @abstractmethod
    def top(cls) -> BoundedLatticeType: ...


class UnionMeta(LatticeMeta):
    """Meta class for union types. It simplifies the union if possible."""

    def __call__(
        self,
        typ: Iterable[LatticeType] | LatticeType,
        *others: LatticeType,
    ):
        from kirin.lattice.abc import Lattice

        if isinstance(typ, Lattice):
            typs: Iterable[LatticeType] = (typ, *others)
        elif not others:
            typs = typ
        else:
            raise ValueError(
                "Expected an iterable of types or variadic arguments of types"
            )

        # try if the union can be simplified
        params: list[LatticeType] = []
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
