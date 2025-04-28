from __future__ import annotations

from abc import ABC, abstractmethod
from typing import TYPE_CHECKING, Union, Generic, TypeVar, Callable, ClassVar
from contextlib import contextmanager
from dataclasses import field, dataclass

from typing_extensions import Self

from kirin import ir

from .frame import FrameABC
from .state import InterpreterState
from .table import Signature, BoundedDef
from .value import Successor, ReturnValue, SpecialValue, StatementResult
from .exceptions import InterpreterError, StackOverflowError

ValueType = TypeVar("ValueType")
FrameType = TypeVar("FrameType", bound=FrameABC)


@dataclass
class InterpreterABC(ABC, Generic[FrameType, ValueType]):
    keys: ClassVar[tuple[str, ...]]

    dialects: ir.DialectGroup
    """The dialects this interpreter supports."""

    max_depth: int = field(default=1000, kw_only=True)
    """The maximum depth of the interpreter stack."""

    registry: dict[Signature, BoundedDef] = field(init=False, compare=False)
    """The registry of implementations"""
    state: InterpreterState
    """The interpreter state."""

    @abstractmethod
    def initialize_frame(self, node: ir.Statement) -> FrameType:
        """Initialize a new call frame for the given callable node."""
        ...

    def call(self, node: ir.Statement, *args: ValueType) -> tuple[FrameType, ValueType]:
        """Call a given callable node.

        This method is used to call a node that has a callable trait and a
        corresponding implementation of its callable region execution convention in
        the interpreter.
        """
        trait = node.get_present_trait(ir.CallableStmtInterface)
        region = trait.get_callable_region(node)
        how = self.registry.get(
            Signature(node.get_present_trait(ir.RegionExecutionInterface))
        )
        if how is None:
            raise InterpreterError(
                f"Interpreter {self.__class__.__name__} does not "
                f"support {node} using {trait} convention"
            )
        return self.call_region(how, node, region, *args)

    def call_region(
        self,
        how: Callable[[Self, FrameType, ir.Region], ValueType],
        node: ir.Statement,
        region: ir.Region,
        *args: ValueType,
    ) -> tuple[FrameType, ValueType]:
        if self.state.depth >= self.max_depth:
            raise StackOverflowError(
                f"Interpreter {self.__class__.__name__} stack "
                f"overflow at {self.state.depth}"
            )

        with self.new_frame(node) as frame:
            return frame, how(self, frame, region)

    @contextmanager
    def new_frame(self, node: ir.Statement):
        """Create a new frame for the given node."""
        frame = self.initialize_frame(node)
        try:
            yield frame
        finally:
            self.state.pop_frame()

    def eval(self, node: ir.Statement) -> ValueType: ...
