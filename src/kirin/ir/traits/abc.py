import ast
from abc import ABC, abstractmethod
from typing import TYPE_CHECKING, Generic, TypeVar
from dataclasses import dataclass

if TYPE_CHECKING:
    from kirin import lowering
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
    """A trait that indicates the properties of the statement's region."""

    @abstractmethod
    def get_graph(self, region: "Region") -> GraphType: ...


ASTNode = TypeVar("ASTNode", bound=ast.AST)
StatementType = TypeVar("StatementType", bound="Statement")


@dataclass(frozen=True)
class PythonLoweringTrait(StmtTrait, Generic[StatementType, ASTNode]):
    """A trait that indicates that a statement can be lowered from Python AST."""

    @abstractmethod
    def lower(
        self, stmt: type[StatementType], state: "lowering.LoweringState", node: ASTNode
    ) -> "lowering.Result": ...
