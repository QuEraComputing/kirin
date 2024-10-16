from __future__ import annotations

from abc import ABC, abstractmethod
from typing import TYPE_CHECKING, Generic, Iterator, TypeVar

from typing_extensions import Self

from kirin.ir.derive import derive
from kirin.ir.ssa import SSAValue
from kirin.print import Printable, Printer

if TYPE_CHECKING:
    from kirin.ir.nodes.stmt import Statement


ParentType = TypeVar("ParentType", bound="IRNode")


@derive(id_hash=True)
class IRNode(Generic[ParentType], ABC, Printable):
    def assert_parent(self, type_: type[IRNode], parent) -> None:
        assert (
            isinstance(parent, type_) or parent is None
        ), f"Invalid parent, expect {type_} or None, got {type(parent)}"

    @property
    @abstractmethod
    def parent_node(self) -> ParentType | None: ...

    @parent_node.setter
    @abstractmethod
    def parent_node(self, parent: ParentType | None) -> None: ...

    def is_ancestor(self, op: IRNode) -> bool:
        if op is self:
            return True
        if (parent := op.parent_node) is None:
            return False
        return self.is_ancestor(parent)

    def get_root(self) -> IRNode:
        if (parent := self.parent_node) is None:
            return self
        return parent.get_root()

    def is_equal(self, other: IRNode, context: dict = {}) -> bool:
        if not isinstance(other, type(self)):
            return False
        return self.is_structurally_equal(other, context)

    def attach(self, parent: ParentType) -> None:
        assert isinstance(parent, IRNode), f"Expected IRNode, got {type(parent)}"

        if self.parent_node:
            raise ValueError("Node already has a parent")
        if self.is_ancestor(parent):
            raise ValueError("Node is an ancestor of the parent")
        self.parent_node = parent

    @abstractmethod
    def clone(self) -> Self: ...

    @abstractmethod
    def detach(self) -> None: ...

    @abstractmethod
    def drop_all_references(self) -> None: ...

    @abstractmethod
    def delete(self, safe: bool = True) -> None: ...

    @abstractmethod
    def is_structurally_equal(
        self,
        other: Self,
        context: dict[IRNode | SSAValue, IRNode | SSAValue] | None = None,
    ) -> bool: ...

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
    def typecheck(self) -> None: ...

    """check if types are correct.
    """

    @abstractmethod
    def verify(self) -> None: ...

    """run mandatory validation checks. This is not same as typecheck, which may be optional.
    """
