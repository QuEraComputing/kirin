from __future__ import annotations

from abc import ABC, ABCMeta, abstractmethod
from dataclasses import dataclass, field, fields
from functools import cached_property
from typing import TYPE_CHECKING, ClassVar, TypeVar

from kirin.lattice import Lattice, LatticeMeta, SingletonMeta, UnionMeta
from kirin.print import Printable, Printer

if TYPE_CHECKING:
    from kirin.ir.dialect import Dialect


TypeLatticeElem = TypeVar("TypeLatticeElem", bound="TypeAttribute", covariant=True)


class AttributeMeta(ABCMeta):
    pass


class TypeAttributeMeta(AttributeMeta, LatticeMeta):
    pass


class SingletonTypeMeta(TypeAttributeMeta, SingletonMeta):
    pass


class UnionTypeMeta(TypeAttributeMeta, UnionMeta):
    pass


@dataclass
class Attribute(ABC, Printable, metaclass=AttributeMeta):
    dialect: ClassVar[Dialect | None] = field(default=None, init=False, repr=False)
    name: ClassVar[str] = field(init=False, repr=False)

    @abstractmethod
    def __hash__(self) -> int: ...


@dataclass(init=False)
class StructAttribute(Attribute, ABC):

    @cached_property
    def parameters(self):
        ret = []
        # return fields of type Attribute
        for f in fields(self):
            if isinstance(f.type, type) and issubclass(f.type, Attribute):
                ret.append(getattr(self, f.name))
        return ret

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self)
        printer.plain_print("(")
        values = {}
        for f in fields(self):
            value = getattr(self, f.name)
            if isinstance(value, Attribute):
                values[f.name] = value
            elif isinstance(value, tuple) and all(
                isinstance(v, Attribute) for v in value
            ):
                values[f.name] = value

        def _emit_attr(attr: tuple[Attribute, ...] | Attribute):
            if isinstance(attr, tuple):
                printer.print_seq(attr, delim=", ", prefix="(", suffix=")")
            else:
                printer.print(attr)

        printer.print_mapping(values, emit=_emit_attr, delim=", ")
        printer.plain_print(")")


@dataclass
class TypeAttribute(Attribute, Lattice[TypeLatticeElem], metaclass=TypeAttributeMeta):

    def __eq__(self, other: object) -> bool:
        return isinstance(other, TypeAttribute) and self.is_equal(other)

    def __or__(self, other: Lattice[TypeLatticeElem]):
        return self.join(other)

    @abstractmethod
    def join(self, other: Lattice[TypeLatticeElem]) -> Lattice[TypeLatticeElem]: ...

    @abstractmethod
    def meet(self, other: Lattice[TypeLatticeElem]) -> Lattice[TypeLatticeElem]: ...

    @classmethod
    def top(cls) -> Lattice[TypeLatticeElem]:
        return AnyType()

    @classmethod
    def bottom(cls) -> Lattice[TypeLatticeElem]:
        return BottomType()

    def is_top(self):
        return isinstance(self, AnyType)

    def is_bottom(self):
        return isinstance(self, BottomType)

    @abstractmethod
    def __hash__(self) -> int: ...

    @abstractmethod
    def is_subseteq(self, other: Lattice[TypeLatticeElem]) -> bool: ...

    def is_subtype(self, other: Lattice[TypeLatticeElem]) -> bool:
        return self.is_subseteq(other)

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self, prefix="!")


class AnyType(TypeAttribute[TypeLatticeElem], metaclass=SingletonTypeMeta):
    """Top of any type lattice."""

    name = "Any"

    def join(
        self: Lattice[TypeLatticeElem], other: Lattice[TypeLatticeElem]
    ) -> Lattice[TypeLatticeElem]:
        return self

    def meet(
        self: Lattice[TypeLatticeElem], other: Lattice[TypeLatticeElem]
    ) -> Lattice[TypeLatticeElem]:
        return other

    def is_subseteq(self, other: Lattice[TypeLatticeElem]) -> bool:
        return isinstance(other, AnyType)  # allow subclassing

    def is_equal(self, other: Lattice[TypeLatticeElem]) -> bool:
        return isinstance(other, AnyType)  # allow subclassing

    def __hash__(self):
        return id(self)


class BottomType(TypeAttribute[TypeLatticeElem], metaclass=SingletonTypeMeta):
    """Bottom of any type lattice."""

    name = "Bottom"

    def join(
        self: Lattice[TypeLatticeElem], other: Lattice[TypeLatticeElem]
    ) -> Lattice[TypeLatticeElem]:
        return other

    def meet(
        self: Lattice[TypeLatticeElem], other: Lattice[TypeLatticeElem]
    ) -> Lattice[TypeLatticeElem]:
        return self

    def is_equal(self, other: Lattice[TypeLatticeElem]) -> bool:
        return isinstance(other, BottomType)  # allow subclassing

    def is_subseteq(self, other: Lattice[TypeLatticeElem]) -> bool:
        return True

    def __hash__(self):
        return id(self)
