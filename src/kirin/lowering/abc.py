from __future__ import annotations

from abc import ABC, abstractmethod
from typing import TYPE_CHECKING, Any, Generic, TypeVar, TypeAlias
from dataclasses import dataclass

from kirin.ir import Region, SSAValue, Statement, DialectGroup

from .exception import BuildError

if TYPE_CHECKING:
    from .state import State


Result: TypeAlias = SSAValue | tuple[SSAValue, ...] | Statement | None
"""Result of lowering a node.
This is used to indicate that the node can be lowered to a SSAValue or None.

If the node is corresponding to a single statement that has a single result value,
the result can also be a Statement for convenience.

If the node can be assigned to a variable syntax-wise, it returns the SSAValue.
If the node cannot be assigned to a variable, it returns None.
"""

EntryNodeType = TypeVar("EntryNodeType")
ASTNodeType = TypeVar("ASTNodeType")


@dataclass
class LoweringABC(ABC, Generic[ASTNodeType]):
    """Base class for lowering.

    This class is used to lower the AST nodes to IR.
    It contains the lowering process and the state of the lowering process.
    """

    dialects: DialectGroup
    """dialects to lower to"""

    @abstractmethod
    def run(
        self,
        stmt: ASTNodeType,
        *,
        source: str | None = None,
        globals: dict[str, Any] | None = None,
        file: str | None = None,
        lineno_offset: int = 0,
        col_offset: int = 0,
        compactify: bool = True,
    ) -> Region: ...

    @abstractmethod
    def visit(self, state: State[ASTNodeType], node: ASTNodeType) -> Result:
        """Entry point of AST visitors.

        Args:
            state: lowering state
            node: AST node to be lowered
        Returns:
            SSAValue: if the node can be assigned to a variable syntax-wise,
                what is the `SSAValue`.
            Statement: if the node is a single statement that has a single result value.
                This is equivalent to returning `stmt.results[0]`.
            None: If the node cannot be assigned to a variable syntax-wise.
        Raises:
            lowering.BuildError: if the node cannot be lowered.
        """
        ...

    @abstractmethod
    def lower_literal(self, state: State[ASTNodeType], value) -> SSAValue: ...

    @dataclass
    class Result:
        data: Any

        ExpectT = TypeVar("ExpectT")

        def expect(self, typ: type[ExpectT]) -> ExpectT:
            if not isinstance(self.data, typ):
                raise BuildError(f"expected {typ}, got {type(self.data)}")
            return self.data

    @abstractmethod
    def lower_global(
        self, state: State[ASTNodeType], node: ASTNodeType
    ) -> LoweringABC.Result:
        """Transform a given global expression to a SSAValue.

        This method is overridden by the subclass to transform a given global
        AST expression to a value as `LoweringABC.Result`.

        The subclass must implement this method to transform a given global
        AST expression to a SSAValue.
        """
        ...

    def lower_global_no_raise(
        self, state: State[ASTNodeType], node: ASTNodeType
    ) -> LoweringABC.Result | None:
        """Transform a given global expression to a SSAValue.

        This method can be overridden by the subclass to transform a given global
        AST expression to a SSAValue.
        """
        try:
            return self.lower_global(state, node)
        except BuildError:
            return None
