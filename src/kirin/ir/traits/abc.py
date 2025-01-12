from abc import ABC, abstractmethod
from typing import TYPE_CHECKING, Generic, TypeVar
from dataclasses import dataclass

if TYPE_CHECKING:
    from kirin.ir import Block, Region, Statement
    from kirin.graph import Graph


@dataclass(frozen=True)
class StmtTrait(ABC):
    """Base class for all statement traits."""

    def verify(self, stmt: "Statement"):
        pass


GraphType = TypeVar("GraphType", bound="Graph[Block]")


@dataclass(frozen=True)
class RegionTrait(StmtTrait, Generic[GraphType]):

    @abstractmethod
    def get_graph(self, region: "Region") -> GraphType: ...
