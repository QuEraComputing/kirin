# TODO: replace with something faster
from typing import Any, Generic, TypeVar, overload
from dataclasses import dataclass

T = TypeVar("T")
L = TypeVar("L")


@dataclass
class IList(Generic[T, L]):
    """A simple immutable list."""

    data: list[T]

    def __len__(self) -> int:
        return len(self.data)

    @overload
    def __add__(self, other: "IList[T, Any]") -> "IList[T, Any]": ...

    @overload
    def __add__(self, other: list[T]) -> "IList[T, Any]": ...

    def __add__(self, other):
        return IList(self.data + other)

    @overload
    def __radd__(self, other: "IList[T, Any]") -> "IList[T, Any]": ...

    @overload
    def __radd__(self, other: list[T]) -> "IList[T, Any]": ...

    def __radd__(self, other):
        return IList(other + self.data)

    def __repr__(self) -> str:
        return f"IList({self.data})"

    def __str__(self) -> str:
        return f"IList({self.data})"

    def __iter__(self):
        raise NotImplementedError("Cannot use IList outside kernel.")

    def __getitem__(self, index: int) -> T:
        return self.data[index]

    def __eq__(self, value: object) -> bool:
        if not isinstance(value, IList):
            return False
        return self.data == value.data
