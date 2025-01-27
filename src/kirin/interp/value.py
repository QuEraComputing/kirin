from typing import Tuple, Generic, TypeVar, TypeAlias, final
from dataclasses import dataclass

from kirin.ir import Block

ValueType = TypeVar("ValueType")


@dataclass(init=False)
class SpecialValue(Generic[ValueType]):
    pass


@final
@dataclass(init=False)
class ReturnValue(SpecialValue[ValueType]):
    """Return value from a statement evaluation."""

    result: ValueType

    def __init__(self, result: ValueType):
        super().__init__()
        self.result = result

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
