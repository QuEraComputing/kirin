"""Lattice for constant analysis."""

from typing import Any, final
from dataclasses import dataclass

from kirin import ir
from kirin.lattice import (
    BoundedLattice,
    IsSubsetEqMixin,
    SimpleJoinMixin,
    SimpleMeetMixin,
)
from kirin.ir.attrs.abc import LatticeAttributeMeta, SingletonLatticeAttributeMeta
from kirin.print.printer import Printer

from ._visitor import _ElemVisitor


@dataclass
class Result(
    ir.Attribute,
    IsSubsetEqMixin["Result"],
    SimpleJoinMixin["Result"],
    SimpleMeetMixin["Result"],
    BoundedLattice["Result"],
    _ElemVisitor,
    metaclass=LatticeAttributeMeta,
):
    """Base class for constant analysis results."""

    @classmethod
    def top(cls) -> "Result":
        return Unknown()

    @classmethod
    def bottom(cls) -> "Result":
        return Bottom()

    def print_impl(self, printer: Printer) -> None:
        printer.plain_print(repr(self))


@final
@dataclass
class Unknown(Result, metaclass=SingletonLatticeAttributeMeta):
    """Unknown constant value. This is the top element of the lattice."""

    def is_subseteq(self, other: Result) -> bool:
        return isinstance(other, Unknown)

    def __hash__(self) -> int:
        return id(self)


@final
@dataclass
class Bottom(Result, metaclass=SingletonLatticeAttributeMeta):
    """Bottom element of the lattice."""

    def is_subseteq(self, other: Result) -> bool:
        return True

    def __hash__(self) -> int:
        return id(self)


@final
@dataclass
class Value(Result):
    """Constant value. Wraps any hashable Python value."""

    data: Any

    def is_subseteq_Value(self, other: "Value") -> bool:
        return self.data == other.data

    def is_equal(self, other: Result) -> bool:
        if isinstance(other, Value):
            return self.data == other.data
        return False

    def __hash__(self) -> int:
        # NOTE: we use id here because the data
        # may not be hashable. This is fine because
        # the data is guaranteed to be unique.
        return id(self)


@dataclass
class PartialConst(Result):
    """Base class for partial constant values."""

    pass


@final
class PartialTupleMeta(LatticeAttributeMeta):
    """Metaclass for PartialTuple.

    This metaclass canonicalizes PartialTuple instances with all Value elements
    into a single Value instance.
    """

    def __call__(cls, data: tuple[Result, ...]):
        if all(isinstance(x, Value) for x in data):
            return Value(tuple(x.data for x in data))  # type: ignore
        return super().__call__(data)


@final
@dataclass
class PartialTuple(PartialConst, metaclass=PartialTupleMeta):
    """Partial tuple constant value."""

    data: tuple[Result, ...]

    def join(self, other: Result) -> Result:
        if other.is_subseteq(self):
            return self
        elif self.is_subseteq(other):
            return other
        elif isinstance(other, PartialTuple):
            return PartialTuple(tuple(x.join(y) for x, y in zip(self.data, other.data)))
        elif isinstance(other, Value) and isinstance(other.data, tuple):
            return PartialTuple(
                tuple(x.join(Value(y)) for x, y in zip(self.data, other.data))
            )
        return Unknown()

    def meet(self, other: Result) -> Result:
        if self.is_subseteq(other):
            return self
        elif other.is_subseteq(self):
            return other
        elif isinstance(other, PartialTuple):
            return PartialTuple(tuple(x.meet(y) for x, y in zip(self.data, other.data)))
        elif isinstance(other, Value) and isinstance(other.data, tuple):
            return PartialTuple(
                tuple(x.meet(Value(y)) for x, y in zip(self.data, other.data))
            )
        return self.bottom()

    def is_equal(self, other: Result) -> bool:
        if isinstance(other, PartialTuple):
            return all(x.is_equal(y) for x, y in zip(self.data, other.data))
        elif isinstance(other, Value) and isinstance(other.data, tuple):
            return all(x.is_equal(Value(y)) for x, y in zip(self.data, other.data))
        return False

    def is_subseteq_PartialTuple(self, other: "PartialTuple") -> bool:
        return all(x.is_subseteq(y) for x, y in zip(self.data, other.data))

    def is_subseteq_Value(self, other: Value) -> bool:
        if isinstance(other.data, tuple):
            return all(x.is_subseteq(Value(y)) for x, y in zip(self.data, other.data))
        return False

    def __hash__(self) -> int:
        return hash(self.data)


@final
@dataclass
class PartialLambda(PartialConst):
    """Partial lambda constant value.

    This represents a closure with captured variables.
    """

    argnames: list[str]
    code: ir.Statement
    captured: tuple[Result, ...]

    def __hash__(self) -> int:
        return hash((self.argnames, self.code, self.captured))

    def is_subseteq_PartialLambda(self, other: "PartialLambda") -> bool:
        if self.code is not other.code:
            return False
        if len(self.captured) != len(other.captured):
            return False

        return all(x.is_subseteq(y) for x, y in zip(self.captured, other.captured))

    def join(self, other: Result) -> Result:
        if other is other.bottom():
            return self

        if not isinstance(other, PartialLambda):
            return Unknown().join(other)  # widen self

        if self.code is not other.code:
            return Unknown()  # lambda stmt is pure

        if len(self.captured) != len(other.captured):
            return self.bottom()  # err

        return PartialLambda(
            self.argnames,
            self.code,
            tuple(x.join(y) for x, y in zip(self.captured, other.captured)),
        )

    def meet(self, other: Result) -> Result:
        if not isinstance(other, PartialLambda):
            return Unknown().meet(other)

        if self.code is not other.code:
            return self.bottom()

        if len(self.captured) != len(other.captured):
            return Unknown()

        return PartialLambda(
            self.argnames,
            self.code,
            tuple(x.meet(y) for x, y in zip(self.captured, other.captured)),
        )
