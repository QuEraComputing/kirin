from abc import ABC, abstractmethod
from typing import Generic, TypeVar, Iterator, Sequence, overload
from dataclasses import dataclass

from typing_extensions import Self

from kirin.ir.ssa import SSAValue
from kirin.ir.nodes.base import IRNode

ElemType = TypeVar("ElemType", bound=IRNode | SSAValue)
FieldType = TypeVar("FieldType", bound=Sequence)
NodeType = TypeVar("NodeType", bound=IRNode | SSAValue)


@dataclass
class View(ABC, Generic[NodeType, ElemType]):
    node: NodeType

    @abstractmethod
    def __iter__(self) -> Iterator[ElemType]: ...

    @abstractmethod
    def __len__(self) -> int: ...

    def __bool__(self) -> bool:
        return bool(len(self))

    def append(self, value: ElemType) -> None:
        raise NotImplementedError

    def extend(self, values: Sequence[ElemType]) -> None:
        for value in values:
            self.append(value)

    def __reversed__(self) -> Iterator[ElemType]:
        raise NotImplementedError


@dataclass
class SequenceView(Generic[FieldType, NodeType, ElemType], View[NodeType, ElemType]):
    field: FieldType

    @classmethod
    def similar(cls, node: NodeType, field: FieldType) -> Self:
        return cls(node, field)

    def __iter__(self) -> Iterator[ElemType]:
        return iter(self.field)

    def __len__(self) -> int:
        return len(self.field)

    def __reversed__(self) -> Iterator[ElemType]:
        return reversed(self.field)

    def isempty(self) -> bool:
        return len(self) == 0

    def __bool__(self) -> bool:
        return not self.isempty()

    # optional interface
    @overload
    def __getitem__(self, idx: int) -> ElemType: ...

    @overload
    def __getitem__(self, idx: slice) -> Self: ...

    def __getitem__(self, idx: int | slice) -> ElemType | Self:
        if isinstance(idx, slice):
            x: FieldType = self.field[idx]  # type: ignore
            return self.similar(self.node, x)
        else:
            return self.field[idx]


@dataclass
class MutableSequenceView(SequenceView[FieldType, NodeType, ElemType]):
    @overload
    def __setitem__(self, idx: int, value: ElemType) -> None: ...

    @overload
    def __setitem__(self, idx: slice, value: Sequence[ElemType]) -> None: ...

    def __setitem__(
        self, idx: int | slice, value: ElemType | Sequence[ElemType]
    ) -> None:
        if isinstance(idx, int) and not isinstance(value, Sequence):
            return self.set_item(idx, value)
        elif isinstance(idx, slice):
            assert isinstance(value, Sequence), "Expected sequence of values"
            if idx.step is not None:  # no need to support step
                raise ValueError("Slice step is not supported")
            return self.set_item_slice(idx, value)
        else:
            raise TypeError("Expected int or slice")

    def set_item(self, idx: int, value: ElemType) -> None:
        raise NotImplementedError

    def set_item_slice(self, s: slice, value: Sequence[ElemType]) -> None:
        # replace the view of slice
        for idx in range(s.start, s.stop):
            if idx < len(value):
                self.set_item(idx, value[idx])
            else:
                del self[idx]

        # insert the rest of the values
        for idx, v in enumerate(value[s.stop - s.start :]):
            self.insert(idx + s.stop, v)

    def __delitem__(self, idx: int) -> None:
        raise NotImplementedError

    def insert(self, idx: int, value: ElemType) -> None:
        raise NotImplementedError

    def pop(self, idx: int = -1) -> ElemType:
        item = self.field[idx]
        del self[idx]
        return item

    def poplast(self) -> ElemType | None:
        """Pop the last element from the view.

        Returns:
            The last element in the view.
        """
        if self:
            return self.pop(-1)
        return None

    def popfirst(self) -> ElemType | None:
        """Pop the first element from the view.

        Returns:
            The first element in the view.
        """
        if self:
            return self.pop(0)
        return None
