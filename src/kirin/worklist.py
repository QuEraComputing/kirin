from __future__ import annotations

from queue import SimpleQueue
from typing import Generic, TypeVar, Iterable

ElemType = TypeVar("ElemType")


class WorkList(SimpleQueue, Generic[ElemType]):
    """The worklist data structure.

    The worklist is a stack that allows for O(1) removal of elements from the stack.
    """

    def __len__(self) -> int:
        return self.qsize()

    def __bool__(self) -> bool:
        return not self.empty()

    def is_empty(self) -> bool:
        return self.empty()

    def append(self, item: ElemType) -> None:
        self.put_nowait(item)

    def extend(self, items: Iterable[ElemType]) -> None:
        for item in items:
            self.put_nowait(item)

    def pop(self) -> ElemType | None:
        if self.empty():
            return None
        return self.get_nowait()


# Remove one function call from critical speed bottleneck
WorkList.is_empty = WorkList.empty
WorkList.append = WorkList.put_nowait
