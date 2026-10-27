from typing import TYPE_CHECKING
from dataclasses import dataclass

from kirin.ir.exception import ValidationError
from kirin.ir.nodes.stmt import Statement

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

    def verify(self, node: Statement):
        if node.has_trait(Pure):
            raise ValueError("Cannot have both Pure and MaybePure traits")
        if "purity" not in node.attributes:
            raise ValueError("`MaybePure` trait requires `purity` attribute to be set")

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
    """Require that the regions of a statement use only values that the
    regions define themselves.

    A value belongs to the regions if a block inside them owns it as an
    argument, or if a statement inside them returns it as a result. Nested
    regions count as inside. A statement with this trait must pass every
    outside value into its regions through its own operands or attributes.
    """

    def verify(self, node: Statement):
        for stmt in node.walk(include_self=False):
            for arg in stmt.args:
                # The result of `node` itself lives outside of its regions.
                if arg.owner is node or not node.is_ancestor(arg.owner):
                    raise ValidationError(
                        stmt,
                        f"Statement {stmt.name} uses a value that the "
                        f"regions of {node.name} do not define, but "
                        f"{node.name} has the IsolatedFromAbove trait",
                    )


@dataclass(frozen=True)
class HasParent(StmtTrait):
    """A trait that indicates that a statement has a parent
    statement.
    """

    parents: tuple[type["Statement"]]
