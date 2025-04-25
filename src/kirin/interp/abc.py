from __future__ import annotations

from abc import ABC
from typing import TYPE_CHECKING, Union, Generic, TypeVar, ClassVar
from dataclasses import field, dataclass

from kirin import ir

from .frame import FrameABC
from .state import InterpreterState
from .table import Signature, BoundedDef
from .value import Successor, ReturnValue, SpecialValue, StatementResult
from .exceptions import InterpreterError

ValueType = TypeVar("ValueType")
FrameType = TypeVar("FrameType", bound=FrameABC)


@dataclass
class InterpreterABC(ABC, Generic[FrameType, ValueType]):
    keys: ClassVar[tuple[str, ...]]

    def call(self, node: ir.Statement, *args: ValueType) -> ValueType:
        from kirin.dialects.func import Invoke

        self.eval(Invoke(args, callee=node))

    def eval(self, node: ir.Statement) -> ValueType: ...
