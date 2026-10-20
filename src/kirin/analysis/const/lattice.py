"""Lattice for constant analysis."""

import math
from typing import Any, final
from dataclasses import dataclass
from collections.abc import Sequence

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

    def __eq__(self, value: object) -> bool:
        return isinstance(value, Unknown)

    def __hash__(self) -> int:
        return id(self)

    def is_structurally_equal(
        self, other: ir.Attribute, context: dict | None = None
    ) -> bool:
        return isinstance(other, Unknown)


@final
@dataclass
class Bottom(Result, metaclass=SingletonLatticeAttributeMeta):
    """Bottom element of the lattice."""

    def is_subseteq(self, other: Result) -> bool:
        return True

    def __eq__(self, value: object) -> bool:
        return isinstance(value, Bottom)

    def __hash__(self) -> int:
        return id(self)

    def is_structurally_equal(
        self, other: ir.Attribute, context: dict | None = None
    ) -> bool:
        return isinstance(other, Bottom)


def _same_constant(lhs: Any, rhs: Any) -> bool:
    """Return True if one constant can replace the other in any program.

    Python `==` is too weak for this. It says that `1 == 1.0`, `True == 1`
    and `0.0 == -0.0`, and a container compares its elements with `==`.
    A fold that uses the wrong one of two such constants changes the result
    of the program.

    Each pair of values must have the same type and satisfy `==`, so the
    check returns True only where `==` also does. The check then adds
    stricter checks for floats, complex numbers, and the container kinds
    that a pure statement can build. For any other type, the type's own
    `==` decides.

    The check returns False for two NaN objects. That answer only makes the
    analysis give Unknown, which is always safe.

    The check walks the two values with a list of pending pairs. A Python
    recursion would fail on a tuple that nests a few hundred levels deep.
    The check visits each pair of objects once, so it ends on any value.
    """
    pending = [(lhs, rhs)]
    # The dict values keep each visited object alive. Python can reuse the
    # id of a dead object, such as the float from `lhs.real`, for a new one.
    visited: dict[tuple[int, int], tuple[Any, Any]] = {}
    while pending:
        lhs, rhs = pending.pop()
        if lhs is rhs or (id(lhs), id(rhs)) in visited:
            continue
        visited[id(lhs), id(rhs)] = (lhs, rhs)
        if type(lhs) is not type(rhs) or not (lhs == rhs):
            return False
        if isinstance(lhs, ir.Data):
            # The structural check of an `IList` also compares its element type.
            if not lhs.is_structurally_equal(rhs):
                return False
            if not isinstance(lhs, Sequence):
                # `unwrap` can return the object itself. The checks above
                # then decide, and the walk stops here.
                pending.append((lhs.unwrap(), rhs.unwrap()))
                continue
        if isinstance(lhs, float):
            # `0.0 == -0.0`, but `math.copysign(1.0, -0.0)` gives `-1.0`.
            if math.copysign(1.0, lhs) != math.copysign(1.0, rhs):
                return False
        elif isinstance(lhs, complex):
            pending.append((lhs.real, rhs.real))
            pending.append((lhs.imag, rhs.imag))
        elif isinstance(lhs, Sequence) and not isinstance(lhs, (str, bytes, range)):
            # A `range` holds only ints, so its own `==` is exact. A check of
            # each element would take one step per element of a huge range.
            if len(lhs) != len(rhs):
                return False
            pending.extend(zip(lhs, rhs))
        elif isinstance(lhs, (set, frozenset, dict)):
            # `lhs == rhs` holds, so each element or key of `lhs` has exactly
            # one `==` match in `rhs`. A dict from each element or key of
            # `rhs` to itself finds that match in one step.
            matches = {item: item for item in rhs}
            for item in lhs:
                match = matches.get(item, _NO_MATCH)
                if match is _NO_MATCH:
                    return False
                pending.append((item, match))
                if isinstance(lhs, dict):
                    pending.append((lhs[item], rhs[match]))
        elif isinstance(lhs, slice):
            pending.append((lhs.start, rhs.start))
            pending.append((lhs.stop, rhs.stop))
            pending.append((lhs.step, rhs.step))
    return True


_NO_MATCH = object()
"""Mark an element or key of `lhs` that has no `==` match in `rhs`."""


@final
@dataclass
class Value(Result):
    """Constant value. Wraps any hashable Python value."""

    data: Any

    def is_subseteq_Value(self, other: "Value") -> bool:
        return _same_constant(self.data, other.data)

    def __eq__(self, value: object) -> bool:
        return isinstance(value, Value) and _same_constant(self.data, value.data)

    def __hash__(self) -> int:
        # NOTE: we use id here because the data
        # may not be hashable. This is fine because
        # the data is guaranteed to be unique.
        return id(self)

    def is_structurally_equal(
        self, other: ir.Attribute, context: dict | None = None
    ) -> bool:
        if not isinstance(other, Value):
            return False
        return _same_constant(self.data, other.data)


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
        # A tuple of a different length has no element-wise join. The `zip`
        # below would drop the extra elements of the longer tuple.
        elif not self._same_length(other):
            return Unknown()
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
        elif not self._same_length(other):
            return self.bottom()
        elif isinstance(other, PartialTuple):
            return PartialTuple(tuple(x.meet(y) for x, y in zip(self.data, other.data)))
        elif isinstance(other, Value) and isinstance(other.data, tuple):
            return PartialTuple(
                tuple(x.meet(Value(y)) for x, y in zip(self.data, other.data))
            )
        return self.bottom()

    def _same_length(self, other: ir.Attribute) -> bool:
        """Return True if `other` is a tuple with as many elements as `self`."""
        if isinstance(other, PartialTuple):
            return len(other.data) == len(self.data)
        if isinstance(other, Value) and isinstance(other.data, tuple):
            return len(other.data) == len(self.data)
        return False

    def is_subseteq_PartialTuple(self, other: "PartialTuple") -> bool:
        if not self._same_length(other):
            return False
        return all(x.is_subseteq(y) for x, y in zip(self.data, other.data))

    def is_subseteq_Value(self, other: Value) -> bool:
        if self._same_length(other):
            return all(x.is_subseteq(Value(y)) for x, y in zip(self.data, other.data))
        return False

    def __eq__(self, value: object) -> bool:
        return isinstance(value, PartialTuple) and self.data == value.data

    def __hash__(self) -> int:
        return hash(self.data)

    def is_structurally_equal(
        self, other: ir.Attribute, context: dict | None = None
    ) -> bool:
        if not self._same_length(other):
            return False
        if isinstance(other, PartialTuple):
            return all(
                x.is_structurally_equal(y, context=context)
                for x, y in zip(self.data, other.data)
            )
        elif isinstance(other, Value) and isinstance(other.data, tuple):
            return all(
                x.is_structurally_equal(Value(y), context=context)
                for x, y in zip(self.data, other.data)
            )
        return False


@final
@dataclass
class PartialLambda(PartialConst):
    """Partial lambda constant value.

    This represents a closure with captured variables.
    """

    code: ir.Statement
    captured: tuple[Result, ...]
    argnames: list[str]

    def __init__(
        self,
        code: ir.Statement,
        captured: tuple[Result, ...],
        argnames: list[str] | None = None,
    ):
        trait = code.get_present_trait(ir.CallableStmtInterface)
        region = trait.get_callable_region(code)
        self.argnames = argnames or [
            arg.name or f"arg_{idx}" for idx, arg in enumerate(region.blocks[0].args)
        ]
        self.code = code
        self.captured = captured

    def __eq__(self, value: object) -> bool:
        return (
            isinstance(value, PartialLambda)
            and self.code is value.code
            and self.argnames == value.argnames
            and self.captured == value.captured
        )

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
            self.code,
            tuple(x.join(y) for x, y in zip(self.captured, other.captured)),
            self.argnames,
        )

    def meet(self, other: Result) -> Result:
        if not isinstance(other, PartialLambda):
            return Unknown().meet(other)

        if self.code is not other.code:
            return self.bottom()

        if len(self.captured) != len(other.captured):
            return Unknown()

        return PartialLambda(
            self.code,
            tuple(x.meet(y) for x, y in zip(self.captured, other.captured)),
            self.argnames,
        )

    def is_structurally_equal(
        self, other: ir.Attribute, context: dict | None = None
    ) -> bool:
        return (
            isinstance(other, PartialLambda)
            and self.code.is_structurally_equal(other.code, context=context)
            and self.argnames == other.argnames
            and len(self.captured) == len(other.captured)
            and all(
                x.is_structurally_equal(y, context=context)
                for x, y in zip(self.captured, other.captured)
            )
        )
