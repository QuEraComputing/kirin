import ast
from abc import ABC, abstractmethod
from typing import TYPE_CHECKING, Generic, TypeVar
from dataclasses import dataclass

if TYPE_CHECKING:
    from kirin import lowering
    from kirin.ir import Block, Region, Statement
    from kirin.graph import Graph


IRNodeType = TypeVar("IRNodeType")


@dataclass(frozen=True)
class Trait(ABC, Generic[IRNodeType]):
    """Base class for all statement traits."""

    def verify(self, node: IRNodeType):
        pass


GraphType = TypeVar("GraphType", bound="Graph[Block]")


@dataclass(frozen=True)
class RegionTrait(Trait["Statement"], Generic[GraphType]):
    """A trait that indicates the properties of the statement's region."""

    @abstractmethod
    def get_graph(self, region: "Region") -> GraphType: ...


ASTNode = TypeVar("ASTNode", bound=ast.AST)
StmtType = TypeVar("StmtType", bound="Statement")


@dataclass(frozen=True)
class PythonLoweringTrait(Trait[StmtType], Generic[StmtType, ASTNode]):
    """A trait that indicates that a statement can be lowered from Python AST."""

    @abstractmethod
    def lower(
        self, stmt: type[StmtType], state: "lowering.LoweringState", node: ASTNode
    ) -> "lowering.Result": ...
