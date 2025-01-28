"""Lattice for constant analysis.
"""

from typing import Any, final
from dataclasses import field, dataclass

from kirin import ir
from kirin.lattice import (
    LatticeMeta,
    SingletonMeta,
    BoundedLattice,
    IsSubsetEqMixin,
    SimpleJoinMixin,
    SimpleMeetMixin,
)

from ._visitor import _ElemVisitor


@dataclass
class Result(
    IsSubsetEqMixin["Result"],
    SimpleJoinMixin["Result"],
    SimpleMeetMixin["Result"],
    BoundedLattice["Result"],
    _ElemVisitor,
):
    """Base class for constant analysis results."""

    @classmethod
    def top(cls) -> "Result":
        return Unknown()

    @classmethod
    def bottom(cls) -> "Result":
        return Bottom()


@final
@dataclass
class Unknown(Result, metaclass=SingletonMeta):
    """Unknown constant value. This is the top element of the lattice."""

    def is_subseteq(self, other: Result) -> bool:
        return isinstance(other, Unknown)


@final
@dataclass
class Bottom(Result, metaclass=SingletonMeta):
    """Bottom element of the lattice."""

    def is_subseteq(self, other: Result) -> bool:
        return True


@final
@dataclass
class Value(Result):
    """Constant value. Wraps any Python value."""

    data: Any

    def is_subseteq_Value(self, other: "Value") -> bool:
        return self.data == other.data

    def is_equal(self, other: Result) -> bool:
        if isinstance(other, Value):
            return self.data == other.data
        return False


@dataclass
class PartialConst(Result):
    """Base class for partial constant values."""

    pass


@final
class PartialTupleMeta(LatticeMeta):
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


@final
@dataclass
class PartialLambda(PartialConst):
    """Partial lambda constant value.

    This represents a closure with captured variables.
    """

    argnames: list[str]
    code: ir.Statement
    captured: tuple[Result, ...]

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


@dataclass(frozen=True)
class Purity(
    SimpleJoinMixin["Purity"], SimpleMeetMixin["Purity"], BoundedLattice["Purity"]
):
    """Base class for purity lattice."""

    @classmethod
    def bottom(cls) -> "Purity":
        return PurityBottom()

    @classmethod
    def top(cls) -> "Purity":
        return NotPure()


@dataclass(frozen=True)
class Pure(Purity, metaclass=SingletonMeta):
    """The result is from a pure function."""

    def is_subseteq(self, other: Purity) -> bool:
        return isinstance(other, (NotPure, Pure))


@dataclass(frozen=True)
class NotPure(Purity, metaclass=SingletonMeta):
    """The result is from an impure function."""

    def is_subseteq(self, other: Purity) -> bool:
        return isinstance(other, NotPure)


@dataclass(frozen=True)
class PurityBottom(Purity, metaclass=SingletonMeta):
    """The bottom element of the purity lattice."""

    def is_subseteq(self, other: Purity) -> bool:
        return True


@dataclass
class JointResult(BoundedLattice["JointResult"]):
    """Joint result of constant value and purity.

    This lattice is used to join the constant value and purity of a function
    during constant propagation analysis. This allows the analysis to track
    both the constant value and the purity of the function, so that the analysis
    can propagate constant values through function calls even if the function
    is only partially pure.
    """

    const: Result
    """The constant value of the result."""
    purity: Purity = field(default_factory=Purity.top)
    """The purity of statement that produces the result."""

    @classmethod
    def from_const(cls, value: Any) -> "JointResult":
        return cls(Value(value), Purity.top())

    @classmethod
    def top(cls) -> "JointResult":
        return cls(Result.top(), Purity.top())

    @classmethod
    def bottom(cls) -> "JointResult":
        return cls(Result.bottom(), Purity.bottom())

    def is_subseteq(self, other: "JointResult") -> bool:
        return self.const.is_subseteq(other.const) and self.purity.is_subseteq(
            other.purity
        )

    def join(self, other: "JointResult") -> "JointResult":
        return JointResult(self.const.join(other.const), self.purity.join(other.purity))

    def meet(self, other: "JointResult") -> "JointResult":
        return JointResult(self.const.meet(other.const), self.purity.join(other.purity))
