from __future__ import annotations

from abc import ABC, abstractmethod
from typing import TYPE_CHECKING, Any, Generic, TypeVar
from dataclasses import dataclass

if TYPE_CHECKING:
    from kirin.ir import SSAValue, Statement, DialectGroup


EntryNodeType = TypeVar("EntryNodeType")


@dataclass
class LoweringABC(ABC, Generic[EntryNodeType]):
    """Base class for lowering.

    This class is used to lower the AST nodes to IR.
    It contains the lowering process and the state of the lowering process.
    """

    dialects: DialectGroup
    """dialects to lower to"""

    @abstractmethod
    def run(
        self,
        stmt: EntryNodeType,
        source: str | None = None,
        globals: dict[str, Any] | None = None,
        lineno_offset: int = 0,
        col_offset: int = 0,
        compactify: bool = True,
    ) -> Statement: ...

    @abstractmethod
    def lower_Constant(self, value) -> SSAValue: ...
