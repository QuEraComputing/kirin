from __future__ import annotations

from typing import Generic, TypeVar, Iterable
from dataclasses import field, dataclass

ElemType = TypeVar("ElemType")


@dataclass
class WorkList(Generic[ElemType]):
    """The worklist data structure.

    The worklist is a stack that allows for O(1) removal of elements from the stack.
    """

    _stack: list[ElemType] = field(default_factory=list, init=False)

    def is_empty(self) -> bool:
        return len(self._stack) == 0

    def append(self, item: ElemType) -> None:
        self._stack.append(item)

    def extend(self, items: Iterable[ElemType]) -> None:
        self._stack.extend(items)

    def pop(self) -> ElemType | None:
        if self._stack:
            return self._stack.pop()
        return None
