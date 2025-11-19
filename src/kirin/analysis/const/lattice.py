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

    def __hash__(self) -> int:
        return id(self)

    def is_structurally_equal(
        self, other: ir.Attribute, context: dict | None = None
    ) -> bool:
        return isinstance(other, Bottom)


@final
@dataclass
class Value(Result):
    """Constant value. Wraps any hashable Python value."""

    data: Any

    def is_subseteq_Value(self, other: "Value") -> bool:
        return self.data == other.data

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
        return self.data == other.data


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

    def is_subseteq_PartialTuple(self, other: "PartialTuple") -> bool:
        return all(x.is_subseteq(y) for x, y in zip(self.data, other.data))

    def is_subseteq_Value(self, other: Value) -> bool:
        if isinstance(other.data, tuple):
            return all(x.is_subseteq(Value(y)) for x, y in zip(self.data, other.data))
        return False

    def __hash__(self) -> int:
        return hash(self.data)

    def is_structurally_equal(
        self, other: ir.Attribute, context: dict | None = None
    ) -> bool:
        if isinstance(other, PartialTuple):
            return all(
                x.is_structurally_equal(y, context=context)
                for x, y in zip(self.data, other.data)
            )
        elif isinstance(other, Value) and isinstance(other.data, tuple):
            return all(
                x.is_structurally_equal(y, context=context)
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

@final
@dataclass
class Predecessor(Result):
    """Predecessor block in CFG."""

    block: ir.Block
    value: Result

    def __hash__(self) -> int:
        return id(self)
    
    def __eq__(self, other: object) -> bool:
        if isinstance(other, Predecessor):
            return self.block.is_structurally_equal(other.block) and self.value == other.value
        else:
            return False

    def is_subseteq(self, other: Result) -> bool:
        if isinstance(other, Predecessor):
            return self.value.is_subseteq(other.value)
        else:
            return self.value.is_subseteq(other)
    
    def join(self, other: Result) -> Result:
        if isinstance(other, Predecessor):
            if self.is_subseteq(other):
                return other.value
            elif other.is_subseteq(self):
                return self.value
            else:
                return Union(predecessors=frozenset({self, other}))
        elif isinstance(other, Union):
            return other.join(self)
        else:
            return self.value.join(other)
                
    def meet(self, other: Result) -> Result:
        if isinstance(other, Predecessor):
            if self.is_subseteq(other):
                return self.value
            elif other.is_subseteq(self):
                return other.value
            else:
                return self.bottom()
        elif isinstance(other, Union):
            return other.meet(self)
        else:
            return self.value.meet(other)

@final
@dataclass
class Union(Result):

    predecessors: frozenset[Predecessor]
    
    def __hash__(self) -> int:
        return id(self)
    
    def is_subseteq(self, other: Result) -> bool:
        if isinstance(other, Union):
            return self.predecessors.issubset(other.predecessors)
        elif isinstance(other, Predecessor):
            return all(pred.is_subseteq(other) for pred in self.predecessors)
        else:
            return super().is_subseteq(other)

    def join(self, other: Result) -> Result:
        if isinstance(other, Union):
            union_preds = self.predecessors.union(other.predecessors)
            return Union(predecessors=union_preds)
        elif isinstance(other, Predecessor):
            union_preds = self.predecessors.union({other})
            return Union(predecessors=union_preds)
        else:
            return Unknown()
    
    def meet(self, other: Result) -> Result:
        if isinstance(other, Union):
            common_preds = self.predecessors.intersection(other.predecessors)
            return Union(predecessors=common_preds)
        elif isinstance(other, Predecessor):
            common_preds = self.predecessors.intersection({other})
            return Union(predecessors=common_preds)
        else:
            return self.bottom()

