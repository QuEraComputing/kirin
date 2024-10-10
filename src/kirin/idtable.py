from dataclasses import dataclass, field
from typing import Generic, TypeVar

T = TypeVar("T")


@dataclass
class IdTable(Generic[T]):
    prefix: str = "%"
    table: dict[T, str] = field(default_factory=dict)
    name_count: dict[str, int] = field(default_factory=dict)
    next_id: int = 0

    def add(self, value: T) -> str:
        id = self.next_id
        if (value_name := getattr(value, "name", None)) is not None:
            curr_ind = self.name_count.get(value_name, 0)
            suffix = f"_{curr_ind}" if curr_ind != 0 else ""
            self.name_count[value_name] = curr_ind + 1
            name = self.prefix + value_name + suffix
            self.table[value] = name
        else:
            name = f"{self.prefix}{id}"
            self.next_id += 1
            self.table[value] = name
        return name

    def __getitem__(self, value: T) -> str:
        if value in self.table:
            return self.table[value]
        else:
            return self.add(value)
