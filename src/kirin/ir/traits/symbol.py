from typing import TYPE_CHECKING
from dataclasses import dataclass

from kirin.exceptions import VerificationError
from kirin.ir.traits.abc import StmtTrait

if TYPE_CHECKING:
    from kirin.ir import Statement
    from kirin.dialects.py.data import PyAttr


@dataclass(frozen=True)
class SymbolOpInterface(StmtTrait):

    def get_sym_name(self, stmt: "Statement") -> "PyAttr[str]":
        sym_name: PyAttr[str] | None = stmt.get_attr_or_prop("sym_name")  # type: ignore
        # NOTE: unlike MLIR or xDSL we do not allow empty symbol names
        if sym_name is None:
            raise ValueError(f"Statement {stmt.name} does not have a symbol name")
        return sym_name

    def verify(self, stmt: "Statement"):
        from kirin.ir.types import String
        from kirin.dialects.py import data

        sym_name = self.get_sym_name(stmt)
        if not (
            isinstance(sym_name, data.PyAttr) and sym_name.type.is_subseteq(String)
        ):
            raise ValueError(f"Symbol name {sym_name} is not a string attribute")


@dataclass(frozen=True)
class SymbolTable(StmtTrait):
    """
    Statement with SymbolTable trait can only have one region with one block.
    """

    @staticmethod
    def walk(stmt: "Statement"):
        return stmt.regions[0].blocks[0].stmts

    def verify(self, stmt: "Statement"):
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
