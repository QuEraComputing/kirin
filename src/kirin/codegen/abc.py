from abc import ABC, abstractmethod
from typing import Generic, TypeVar
from dataclasses import field, dataclass

from kirin import ir, interp

ValueType = TypeVar("ValueType")


@dataclass
class Frame(interp.FrameABC[ir.SSAValue | ir.Block, ValueType]):
    ssa: dict[ir.SSAValue, ValueType] = field(default_factory=dict, kw_only=True)
    block: dict[ir.Block, ValueType] = field(default_factory=dict, kw_only=True)

    @abstractmethod
    def new(self, key: ir.SSAValue | ir.Block) -> ValueType: ...

    def get(self, key: ir.SSAValue | ir.Block) -> ValueType:
        if isinstance(key, ir.Block):
            return self.__get_item(self.block, key)
        else:
            return self.__get_item(self.ssa, key)

    KeyType = TypeVar("KeyType", bound=ir.SSAValue | ir.Block)

    def __get_item(self, entries: dict[KeyType, ValueType], key: KeyType) -> ValueType:
        value = entries.get(key, interp.Undefined)
        if interp.is_undefined(value):
            value = self.new(key)
            entries[key] = value
            return value
        return value

    def set(self, key: ir.SSAValue | ir.Block, value: ValueType) -> None:
        if isinstance(key, ir.Block):
            self.block[key] = value
        else:
            self.ssa[key] = value


FrameType = TypeVar("FrameType", bound=Frame)


@dataclass
class CodegenABC(interp.BaseInterpreter[FrameType, ValueType], ABC):
    pass
