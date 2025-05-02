from __future__ import annotations

from abc import abstractmethod
from typing import TYPE_CHECKING, Generic, TypeVar
from dataclasses import dataclass

from kirin.ir.attrs.py import PyAttr
from kirin.ir.exception import ValidationError
from kirin.ir.traits.abc import StmtTrait

if TYPE_CHECKING:
    from kirin.ir import Statement


@dataclass(frozen=True)
class SymbolOpInterface(StmtTrait):
    """A trait that indicates that a statement is a symbol operation.

    A symbol operation is a statement that has a symbol name attribute.
    """

    def get_sym_name(self, stmt: Statement) -> PyAttr[str]:
        sym_name: PyAttr[str] | None = stmt.get_attribute("sym_name")  # type: ignore
        # NOTE: unlike MLIR or xDSL we do not allow empty symbol names
        if sym_name is None:
            raise ValueError(f"Statement {stmt.name} does not have a symbol name")
        return sym_name

    def verify(self, node: Statement):
        from kirin.types import String

        sym_name = self.get_sym_name(node)
        if not (isinstance(sym_name, PyAttr) and sym_name.type.is_subseteq(String)):
            raise ValueError(f"Symbol name {sym_name} is not a string attribute")


@dataclass(frozen=True)
class SymbolTable(StmtTrait):
    """
    Statement with SymbolTable trait can only have one region with one block.
    """

    @staticmethod
    def walk(stmt: Statement):
        for stmt in stmt.regions[0].blocks[0].stmts:
            if stmt.has_trait(SymbolOpInterface):
                yield stmt

    def verify(self, node: Statement):
        if len(node.regions) != 1:
            raise ValidationError(
                node,
                f"Statement {node.name} with SymbolTable trait must have exactly one region",
            )

        if len(node.regions[0].blocks) != 1:
            raise ValidationError(
                node,
                f"Statement {node.name} with SymbolTable trait must have exactly one block",
            )

        # TODO: check uniqueness of symbol names


StmtType = TypeVar("StmtType", bound="Statement")


@dataclass(frozen=True)
class EntryPointInterface(StmtTrait, Generic[StmtType]):
    """A trait that indicates that a module-like statement has an entry point.

    An entry point is a statement that has a symbol name attribute and is
    the first statement in the module.

    When interpreting statements with this trait, the interpreter will
    look for the entry point and start calling the module from there.
    """

    @abstractmethod
    def get_entry_point_symbol(self, stmt: StmtType) -> str: ...

    @abstractmethod
    def get_entry_point(self, stmt: StmtType) -> Statement: ...

    def verify(self, node: Statement):
        if not node.has_trait(SymbolOpInterface):
            raise ValidationError(
                node,
                f"Statement {node.name} with EntryPointInterface trait must have a symbol name",
            )
