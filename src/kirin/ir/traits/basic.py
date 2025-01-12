from typing import TYPE_CHECKING
from dataclasses import dataclass

from .abc import StmtTrait

if TYPE_CHECKING:
    from kirin.ir import Statement


@dataclass(frozen=True)
class Pure(StmtTrait):
    pass


@dataclass(frozen=True)
class ConstantLike(StmtTrait):
    pass


@dataclass(frozen=True)
class IsTerminator(StmtTrait):
    pass


@dataclass(frozen=True)
class NoTerminator(StmtTrait):
    pass


@dataclass(frozen=True)
class IsolatedFromAbove(StmtTrait):
    pass


@dataclass(frozen=True)
class HasParent(StmtTrait):
    parents: tuple[type["Statement"]]
