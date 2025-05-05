from __future__ import annotations

from abc import ABC
from dataclasses import dataclass, field
from typing import TypeVar, TypeAlias

from kirin import ir
from kirin.interp import abc, Frame

TargetType = TypeVar("TargetType")


@dataclass
class EmitFrame(Frame[TargetType]):
    pass


CodeGenFrameType = TypeVar("CodeGenFrameType", bound=EmitFrame)


@dataclass
class EmitABC(abc.InterpreterABC[CodeGenFrameType, TargetType], ABC):

    def __init_subclass__(cls) -> None:
        if ABC in cls.__bases__:
            return super().__init_subclass__()

        cls.keys += ("codegen",)
        super().__init_subclass__()
