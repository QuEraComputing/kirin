from typing import Generic, TypeVar
from dataclasses import field, dataclass

T = TypeVar("T")


@dataclass
class IdTable(Generic[T]):
    """A table that maps values to "human readable" unique names.
    This is used for IR printing and code generation of SSA values
    and basic blocks, or anything else required to have a unique name.

    ## Example

    ```python
    from kirin import ir
    from kirin.idtable import IdTable
    table = IdTable()
    x = ir.TestValue()
    table[x] # "%0"
    table[x] # "%0"
    y = ir.TestValue()
    table[y] # "%1"
    ```
    """

    prefix: str = "%"
    """The prefix to use for generated names."""
    table: dict[T, str] = field(default_factory=dict)
    """The table that maps values to names."""
    name_count: dict[str, int] = field(default_factory=dict)
    """The count of names that have been generated."""
    next_id: int = 0
    """The next ID to use for generating names."""
    prefix_if_none: str = ""
    """An alternate prefix to use when the name is None."""

    def add(self, value: T) -> str:
        """Add a value to the table and return the name."""
        id = self.next_id
        if (value_name := getattr(value, "name", None)) is not None:
            curr_ind = self.name_count.get(value_name, 0)
            suffix = f"_{curr_ind}" if curr_ind != 0 else ""
            self.name_count[value_name] = curr_ind + 1
            name = self.prefix + value_name + suffix
            self.table[value] = name
        else:
            name = f"{self.prefix}{self.prefix_if_none}{id}"
            self.next_id += 1
            self.table[value] = name
        return name

    def __getitem__(self, value: T) -> str:
        if value in self.table:
            return self.table[value]
        else:
            return self.add(value)
