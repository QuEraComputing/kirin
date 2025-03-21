from typing import TYPE_CHECKING, TypeVar
from dataclasses import dataclass

from kirin.exceptions import VerificationError
from kirin.ir.attrs.py import PyAttr
from kirin.ir.traits.abc import Trait

if TYPE_CHECKING:
    from kirin.ir import Statement

StmtType = TypeVar("StmtType", bound="Statement")


@dataclass(frozen=True)
class SymbolOpInterface(Trait[StmtType]):
    """A trait that indicates that a statement is a symbol operation.

    A symbol operation is a statement that has a symbol name attribute.
    """

    def get_sym_name(self, stmt: StmtType) -> "PyAttr[str]":
        sym_name: PyAttr[str] | None = stmt.get_attr_or_prop("sym_name")  # type: ignore
        # NOTE: unlike MLIR or xDSL we do not allow empty symbol names
        if sym_name is None:
            raise ValueError(f"Statement {stmt.name} does not have a symbol name")
        return sym_name

    def verify(self, stmt: StmtType):
        from kirin.types import String

        sym_name = self.get_sym_name(stmt)
        if not (isinstance(sym_name, PyAttr) and sym_name.type.is_subseteq(String)):
            raise ValueError(f"Symbol name {sym_name} is not a string attribute")


@dataclass(frozen=True)
class SymbolTable(Trait[StmtType]):
    """
    Statement with SymbolTable trait can only have one region with one block.
    """

    @staticmethod
    def walk(stmt: StmtType):
        return stmt.regions[0].blocks[0].stmts

    def verify(self, stmt: StmtType):
        if len(stmt.regions) != 1:
            raise VerificationError(
                stmt,
                f"Statement {stmt.name} with SymbolTable trait must have exactly one region",
            )

        if len(stmt.regions[0].blocks) != 1:
            raise VerificationError(
                stmt,
                f"Statement {stmt.name} with SymbolTable trait must have exactly one block",
            )

        # TODO: check uniqueness of symbol names
