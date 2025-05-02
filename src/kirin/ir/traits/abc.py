from __future__ import annotations

from abc import ABC, abstractmethod
from typing import TYPE_CHECKING, Generic, TypeVar
from dataclasses import dataclass

if TYPE_CHECKING:
    from kirin import ir, interp
    from kirin.graph import Graph


IRNodeType = TypeVar("IRNodeType")


@dataclass(frozen=True)
class Trait(ABC, Generic[IRNodeType]):
    """Base class for all statement traits."""

    def verify(self, node: IRNodeType):
        pass


@dataclass(frozen=True)
class AttrTrait(Trait["ir.Attribute"]):
    """Base class for all attribute traits."""

    def verify(self, node: ir.Attribute):
        pass


@dataclass(frozen=True)
class StmtTrait(Trait["ir.Statement"], ABC):
    """Base class for all statement traits."""

    def verify(self, node: ir.Statement):
        pass


GraphType = TypeVar("GraphType", bound="Graph[ir.Block]")


@dataclass(frozen=True)
class RegionGraph(StmtTrait, Generic[GraphType]):
    """A trait that indicates the properties of the statement's region."""

    @abstractmethod
    def get_graph(self, region: ir.Region) -> GraphType: ...


@dataclass(frozen=True)
class RegionInterpretationTrait(StmtTrait):
    """A trait that indicates the execution convention of the statement's region.

    For example, a region is SSA CFG if it follows SSA form and has a control flow graph.
    This trait also indicates that there is an available implementation of the trait in each
    interpreter.
    """

    ValueType = TypeVar("ValueType")

    @classmethod
    @abstractmethod
    def set_region_input(
        cls,
        frame: interp.FrameABC[ir.SSAValue, ValueType],
        region: ir.Region,
        *inputs: ValueType,
    ) -> None: ...
