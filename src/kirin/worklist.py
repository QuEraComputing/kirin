from __future__ import annotations

from dataclasses import dataclass, field
from typing import Generic, Iterable, TypeVar

ElemType = TypeVar("ElemType")


@dataclass
class WorkList(Generic[ElemType]):
    """The worklist data structure.

    The worklist is a stack that allows for O(1) removal of elements from the stack.
    """

    # this is from XDSL
    _stack: list[ElemType | None] = field(default_factory=list, init=False)
    _map: dict[ElemType, int] = field(default_factory=dict, init=False)
    # allow O(1) removal from the stack via storage of the index

    def __contains__(self, item: ElemType) -> bool:
        return item in self._map

    def purge(self) -> None:
        """remove all None values from the stack"""
        while self._stack and self._stack[-1] is None:
            self._stack.pop()

    def is_empty(self) -> bool:
        self.purge()
        return len(self._stack) == 0

    def push(self, item: ElemType) -> None:
        if item not in self._map:
            self._map[item] = len(self._stack)
            self._stack.append(item)

    def append(self, items: Iterable[ElemType]) -> None:
        for item in items:
            self.push(item)

    def pop(self) -> ElemType | None:
        while self._stack:
            item = self._stack.pop()
            if item is not None:
                del self._map[item]
                return item
        return None

    def remove(self, item: ElemType) -> None:
        if item in self._map:
            idx = self._map[item]
            self._stack[idx] = None
            del self._map[item]
