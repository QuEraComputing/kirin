from typing import TYPE_CHECKING
from dataclasses import dataclass

from .abc import StmtTrait

if TYPE_CHECKING:
    from kirin.ir import Statement


@dataclass(frozen=True)
class Pure(StmtTrait):
    """A trait that indicates that a statement is pure, i.e., it has no side
    effects.
    """

    pass


@dataclass(frozen=True)
class MaybePure(StmtTrait):
    """A trait that indicates the statement may be pure,
    i.e., a call statement can be pure if the callee is pure.
    """

    @classmethod
    def is_pure(cls, stmt: "Statement") -> bool:
        # TODO: simplify this after removing property
        from kirin.ir.attrs.py import PyAttr

        purity = stmt.attributes.get("purity")
        if isinstance(purity, PyAttr) and purity.data:
            return True
        return False

    @classmethod
    def set_pure(cls, stmt: "Statement") -> None:
        from kirin.ir.attrs.py import PyAttr

        stmt.attributes["purity"] = PyAttr(True)


@dataclass(frozen=True)
class ConstantLike(StmtTrait):
    """A trait that indicates that a statement is constant-like, i.e., it
    represents a constant value.
    """

    pass


@dataclass(frozen=True)
class IsTerminator(StmtTrait):
    """A trait that indicates that a statement is a terminator, i.e., it
    terminates a block.
    """

    pass


@dataclass(frozen=True)
class NoTerminator(StmtTrait):
    """A trait that indicates that the region of a statement has no terminator."""

    pass


@dataclass(frozen=True)
class IsolatedFromAbove(StmtTrait):
    pass


@dataclass(frozen=True)
class HasParent(StmtTrait):
    """A trait that indicates that a statement has a parent
    statement.
    """

    parents: tuple[type["Statement"]]
