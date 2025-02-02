from typing import Tuple, Generic, TypeVar, TypeAlias, final
from dataclasses import dataclass

from kirin.ir import Block

ValueType = TypeVar("ValueType")


@final
@dataclass
class ReturnValue(Generic[ValueType]):
    """Return value from a statement evaluation.

    This class represents a return value from a statement evaluation. It is used
    to indicate that the statement evaluation should later pop the frame and
    return the value. Kirin does not allow multiple return values to follow Python
    semantics. If you want to return multiple values, you should return a tuple.
    """

    value: ValueType

    def __len__(self) -> int:
        return 0


@final
@dataclass
class YieldValue(Generic[ValueType]):
    """Yield value from a statement evaluation.

    This class represents values returned from a statement that terminates current
    region execution and returns the values to the caller. Unlike `ReturnValue`, this
    class won't pop the frame and return the value to the caller.
    """

    values: tuple[ValueType, ...]

    def __len__(self) -> int:
        return len(self.values)

    def __iter__(self):
        return iter(self.values)

    def __getitem__(self, index: int) -> ValueType:
        return self.values[index]


@final
@dataclass(init=False)
class Successor(Generic[ValueType]):
    """Successor block from a statement evaluation."""

    block: Block
    block_args: Tuple[ValueType, ...]

    def __init__(self, block: Block, *block_args: ValueType):
        super().__init__()
        self.block = block
        self.block_args = block_args

    def __hash__(self) -> int:
        return hash(self.block)

    def __len__(self) -> int:
        return 0


SpecialValue: TypeAlias = (
    None | ReturnValue[ValueType] | YieldValue[ValueType] | Successor[ValueType]
)
"""Special value for statement evaluation."""
StatementResult: TypeAlias = tuple[ValueType, ...] | SpecialValue[ValueType]
"""Type alias for the result of a statement evaluation."""
