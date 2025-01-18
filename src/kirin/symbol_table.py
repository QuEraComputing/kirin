from typing import Generic, TypeVar
from dataclasses import field, dataclass

T = TypeVar("T")


@dataclass
class SymbolTable(Generic[T]):
    names: dict[str, T] = field(default_factory=dict)
    """The table that maps names to values."""
    prefix: str = field(default="", kw_only=True)
    name_count: dict[str, int] = field(default_factory=dict, kw_only=True)
    """The count of names that have been requested."""

    def __getitem__(self, name: str) -> T:
        return self.names[name]

    def __contains__(self, name: str) -> bool:
        return name in self.names

    def __setitem__(self, name: str, value: T) -> None:
        count = self.name_count.setdefault(name, 0)
        self.name_count[name] = count + 1
        self.names[f"{self.prefix}_{name}_{count}"] = value

    def __delitem__(self, name: str) -> None:
        del self.names[name]
