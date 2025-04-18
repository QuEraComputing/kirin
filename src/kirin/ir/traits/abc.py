from __future__ import annotations

from abc import ABC, abstractmethod
from typing import TYPE_CHECKING, Generic, TypeVar
from dataclasses import dataclass

if TYPE_CHECKING:
    from kirin.ir import Block, Region, Attribute, Statement
    from kirin.graph import Graph


IRNodeType = TypeVar("IRNodeType")


@dataclass(frozen=True)
class Trait(ABC, Generic[IRNodeType]):
    """Base class for all statement traits."""

    def verify(self, node: IRNodeType):
        pass


@dataclass(frozen=True)
class AttrTrait(Trait["Attribute"]):
    """Base class for all attribute traits."""

    def verify(self, node: "Attribute"):
        pass


@dataclass(frozen=True)
class StmtTrait(Trait["Statement"], ABC):
    """Base class for all statement traits."""

    def verify(self, node: "Statement"):
        pass


GraphType = TypeVar("GraphType", bound="Graph[Block]")


@dataclass(frozen=True)
class RegionTrait(StmtTrait, Generic[GraphType]):
    """A trait that indicates the properties of the statement's region."""

    @abstractmethod
    def get_graph(self, region: Region) -> GraphType: ...
