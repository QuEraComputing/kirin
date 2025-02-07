from __future__ import annotations

from abc import ABC, abstractmethod
from typing import TYPE_CHECKING, Generic, TypeVar, Iterator
from dataclasses import dataclass

from typing_extensions import Self

from kirin.print import Printer, Printable
from kirin.ir.ssa import SSAValue

if TYPE_CHECKING:
    from kirin.ir.nodes.stmt import Statement


ParentType = TypeVar("ParentType", bound="IRNode")


@dataclass
class IRNode(Generic[ParentType], ABC, Printable):
    """Base class for all IR nodes. All IR nodes are hashable and can be compared
    for equality. The hash of an IR node is the same as the id of the object.

    !!! note "Pretty Printing"
        This object is pretty printable via
        [`.print()`][kirin.print.printable.Printable.print] method.
    """

    def assert_parent(self, type_: type[IRNode], parent) -> None:
        assert (
            isinstance(parent, type_) or parent is None
        ), f"Invalid parent, expect {type_} or None, got {type(parent)}"

    @property
    @abstractmethod
    def parent_node(self) -> ParentType | None:
        """Parent node of the current node."""
        ...

    @parent_node.setter
    @abstractmethod
    def parent_node(self, parent: ParentType | None) -> None: ...

    def is_ancestor(self, op: IRNode) -> bool:
        """Check if the given node is an ancestor of the current node."""
        if op is self:
            return True
        if (parent := op.parent_node) is None:
            return False
        return self.is_ancestor(parent)

    def get_root(self) -> IRNode:
        """Get the root node of the current node."""
        if (parent := self.parent_node) is None:
            return self
        return parent.get_root()

    def is_equal(self, other: IRNode, context: dict = {}) -> bool:
        """Check if the current node is equal to the other node.

        Args:
            other: The other node to compare.
            context: The context to store the visited nodes. Defaults to {}.

        Returns:
            True if the nodes are equal, False otherwise.

        !!! note
            This method is not the same as the `==` operator. It checks for
            structural equality rather than identity. To change the behavior
            of structural equality, override the `is_structurally_equal` method.
        """
        if not isinstance(other, type(self)):
            return False
        return self.is_structurally_equal(other, context)

    def attach(self, parent: ParentType) -> None:
        """Attach the current node to the parent node."""
        assert isinstance(parent, IRNode), f"Expected IRNode, got {type(parent)}"

        if self.parent_node:
            raise ValueError("Node already has a parent")
        if self.is_ancestor(parent):
            raise ValueError("Node is an ancestor of the parent")
        self.parent_node = parent

    @abstractmethod
    def detach(self) -> None:
        """Detach the current node from the parent node."""
        ...

    @abstractmethod
    def drop_all_references(self) -> None:
        """Drop all references to other nodes."""
        ...

    @abstractmethod
    def delete(self, safe: bool = True) -> None:
        """Delete the current node.

        Args:
            safe: If True, check if the node has any references before deleting.
        """
        ...

    @abstractmethod
    def is_structurally_equal(
        self,
        other: Self,
        context: dict[IRNode | SSAValue, IRNode | SSAValue] | None = None,
    ) -> bool:
        """Check if the current node is structurally equal to the other node.

        !!! note
            This method is for tweaking the behavior of structural equality.
            To check if two nodes are structurally equal, use the `is_equal` method.

        Args:
            other: The other node to compare.
            context: The context to store the visited nodes.

        Returns:
            True if the nodes are structurally equal, False otherwise.
        """
        ...

    def __eq__(self, other) -> bool:
        return self is other

    def __hash__(self) -> int:
        return id(self)

    @abstractmethod
    def walk(
        self, *, reverse: bool = False, region_first: bool = False
    ) -> Iterator[Statement]: ...

    @abstractmethod
    def print_impl(self, printer: Printer) -> None: ...

    @abstractmethod
    def typecheck(self) -> None:
        """check if types are correct."""
        ...

    @abstractmethod
    def verify(self) -> None:
        """run mandatory validation checks. This is not same as typecheck, which may be optional."""
        ...
