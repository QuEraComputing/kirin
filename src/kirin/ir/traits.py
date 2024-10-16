from __future__ import annotations

from abc import ABC
from typing import TYPE_CHECKING, Generic, TypeVar

from kirin.exceptions import VerificationError
from kirin.ir.derive import derive

if TYPE_CHECKING:
    from kirin.dialects.func.attrs import Signature
    from kirin.dialects.py.data import PyAttr
    from kirin.ir import Region, Statement


@derive(init=True, repr=True, frozen=True)
class StmtTrait(ABC):

    def verify(self, stmt: Statement):
        pass


@derive(init=True, repr=True, frozen=True)
class Pure(StmtTrait):
    pass


@derive(init=True, repr=True, frozen=True)
class ConstantLike(StmtTrait):
    pass


@derive(init=True, repr=True, frozen=True)
class IsTerminator(StmtTrait):
    pass


@derive(init=True, repr=True, frozen=True)
class NoTerminator(StmtTrait):
    pass


@derive(init=True, repr=True, frozen=True)
class IsolatedFromAbove(StmtTrait):
    pass


@derive(init=True, repr=True, frozen=True)
class HasParent(StmtTrait):
    parents: tuple[type[Statement]]


@derive(init=True, repr=True, frozen=True)
class SymbolOpInterface(StmtTrait):

    def get_sym_name(self, stmt: Statement) -> PyAttr[str]:
        sym_name: PyAttr[str] | None = stmt.get_attr_or_prop("sym_name")  # type: ignore
        # NOTE: unlike MLIR or xDSL we do not allow empty symbol names
        if sym_name is None:
            raise ValueError(f"Statement {stmt.name} does not have a symbol name")
        return sym_name

    def verify(self, stmt: Statement):
        from kirin.dialects.py import data, types

        sym_name = self.get_sym_name(stmt)
        if not (
            isinstance(sym_name, data.PyAttr) and sym_name.type.is_subtype(types.String)
        ):
            raise ValueError(f"Symbol name {sym_name} is not a string attribute")


StmtType = TypeVar("StmtType", bound="Statement")


@derive(init=True, repr=True, frozen=True)
class CallableStmtInterface(StmtTrait, Generic[StmtType]):

    @classmethod
    def get_callable_region(cls, stmt: "StmtType") -> Region:  # type: ignore
        """Returns the body of the callable region"""
        raise NotImplementedError


@derive(init=True, repr=True, frozen=True)
class HasSignature(StmtTrait, ABC):

    @classmethod
    def get_signature(cls, stmt: Statement):
        signature: Signature | None = stmt.attributes.get("signature")  # type: ignore
        if signature is None:
            raise ValueError(f"Statement {stmt.name} does not have a function type")

        return signature

    @classmethod
    def set_signature(cls, stmt: Statement, signature: Signature):
        stmt.attributes["signature"] = signature

    def verify(self, stmt: Statement):
        from kirin.dialects.func.attrs import Signature

        signature = self.get_signature(stmt)
        if not isinstance(signature, Signature):
            raise ValueError(f"{signature} is not a Signature attribute")


@derive(init=True, repr=True, frozen=True)
class SymbolTable(StmtTrait):
    """
    Statement with SymbolTable trait can only have one region with one block.
    """

    @staticmethod
    def walk(stmt: Statement):
        return stmt.regions[0].blocks[0].stmts

    def verify(self, stmt: Statement):
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
