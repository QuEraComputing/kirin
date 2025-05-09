from __future__ import annotations

from abc import ABC, abstractmethod
from typing import TypeVar
from dataclasses import dataclass

from kirin import ir
from kirin.interp import Frame, abc

TargetType = TypeVar("TargetType")


@dataclass
class EmitFrame(Frame[TargetType]):
    pass


CodeGenFrameType = TypeVar("CodeGenFrameType", bound=EmitFrame)


@dataclass
class EmitABC(abc.InterpreterABC[CodeGenFrameType, TargetType], ABC):

    def __init_subclass__(cls) -> None:
        super().__init_subclass__()
        for each in getattr(cls, "keys", ()):
            if not each.startswith("emit."):
                raise ValueError(f"Key {each} cannot start with 'emit.'")

    @abstractmethod
    def run(self, node: ir.Method | ir.Statement): ...
