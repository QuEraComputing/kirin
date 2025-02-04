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
