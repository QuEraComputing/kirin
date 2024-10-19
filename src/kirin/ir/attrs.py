from __future__ import annotations

from abc import ABC, ABCMeta, abstractmethod
from dataclasses import dataclass, field, fields
from functools import cached_property
from typing import TYPE_CHECKING, ClassVar, TypeVar

from kirin.lattice import Lattice, LatticeMeta, SingletonMeta, UnionMeta
from kirin.print import Printable, Printer

if TYPE_CHECKING:
    from kirin.ir.dialect import Dialect


TypeLatticeParent = TypeVar("TypeLatticeParent", bound="TypeAttribute")


class AttributeMeta(ABCMeta):
    pass


class TypeAttributeMeta(AttributeMeta, LatticeMeta):
    pass


class SingletonTypeMeta(TypeAttributeMeta, SingletonMeta):
    pass


class UnionTypeMeta(TypeAttributeMeta, UnionMeta):
    pass


@dataclass(eq=False)
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


@dataclass(eq=False)
class TypeAttribute(Attribute, Lattice["TypeAttribute"], metaclass=TypeAttributeMeta):

    @property
    def parent_type(self) -> type[TypeAttribute]:
        return TypeAttribute

    def __or__(self, other: TypeAttribute):
        return self.join(other)

    @classmethod
    def top(cls) -> AnyType:
        return AnyType()

    @classmethod
    def bottom(cls) -> BottomType:
        return BottomType()

    def is_top(self):
        return isinstance(self, AnyType)

    def is_bottom(self):
        return isinstance(self, BottomType)

    def is_subtype(self, other: TypeAttribute) -> bool:
        return self.is_subseteq(other)

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self, prefix="!")


class AnyType(TypeAttribute, metaclass=SingletonTypeMeta):
    """Top of any type lattice."""

    name = "Any"

    def join(self: TypeAttribute, other: TypeAttribute) -> TypeAttribute:
        return self

    def meet(self: TypeAttribute, other: TypeAttribute) -> TypeAttribute:
        return other

    def is_subseteq(self, other: TypeAttribute) -> bool:
        return isinstance(other, AnyType)  # allow subclassing

    def is_equal(self, other: TypeAttribute) -> bool:
        return other.is_top()

    def __hash__(self):
        return id(self)


class BottomType(TypeAttribute, metaclass=SingletonTypeMeta):
    """Bottom of any type lattice."""

    name = "Bottom"

    def join(self: TypeAttribute, other: TypeAttribute) -> TypeAttribute:
        return other

    def meet(self: TypeAttribute, other: TypeAttribute) -> TypeAttribute:
        return self

    def is_equal(self, other: TypeAttribute) -> bool:
        return other.is_bottom()

    def is_subseteq(self, other: TypeAttribute) -> bool:
        return True

    def __hash__(self):
        return id(self)
