from typing import Generic, TypeVar
from dataclasses import field, dataclass

T = TypeVar("T")


@dataclass
class IntIdTable(Generic[T]):
    """
    A simple SSAValue ID Table that assigns a unique integer ID to each SSAValue.
    """

    current_cnt: int = 0
    lookup: dict[T, int] = field(default_factory=dict)

    def __getitem__(self, key: T) -> int:
        if key in self.lookup:
            return self.lookup[key]

        self.lookup[key] = self.current_cnt
        self.current_cnt += 1
        return self.lookup[key]

    def clear(self) -> None:
        """
        Clear the ID table, resetting the current count and loop-up dictionary.
        """
        self.current_cnt = 0
        self.lookup.clear()
