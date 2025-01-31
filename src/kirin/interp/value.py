from typing import Tuple, Generic, TypeVar, TypeAlias, final
from dataclasses import dataclass

from kirin.ir import Block

ValueType = TypeVar("ValueType")


@dataclass(init=False)
class SpecialValue(Generic[ValueType]):
    """Special value for statement evaluation.

    This class represents a special value that can be returned from a statement
    evaluation. It is used to represent special cases like return values and
    successor blocks.
    """

    pass


@final
@dataclass(init=False)
class ReturnValue(SpecialValue[ValueType]):
    """Return value from a statement evaluation."""

    results: tuple[ValueType, ...]

    def __init__(self, *result: ValueType):
        self.results = result

    def __len__(self) -> int:
        return 0


@final
@dataclass(init=False)
class Successor(SpecialValue[ValueType]):
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


StatementResult: TypeAlias = tuple[ValueType, ...] | SpecialValue[ValueType]
"""Type alias for the result of a statement evaluation."""
