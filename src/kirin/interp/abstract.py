from __future__ import annotations

from abc import ABC
from typing import TypeVar, TypeAlias, overload
from dataclasses import field, dataclass

from kirin import ir
from kirin.lattice import BoundedLattice
from kirin.worklist import WorkList

from .abc import InterpreterABC
from .frame import Frame
from .value import Successor, YieldValue, ReturnValue
from .exceptions import InterpreterError

ResultType = TypeVar("ResultType", bound=BoundedLattice)
WorkListType = TypeVar("WorkListType", bound=WorkList[Successor])
AbsIntResultType: TypeAlias = (
    tuple[ResultType, ...] | None | ReturnValue[ResultType] | YieldValue[ResultType]
)


@dataclass
class AbstractFrame(Frame[ResultType]):
    """Interpreter frame for abstract interpreter.

    This frame is used to store the state of the abstract interpreter.
    It contains the worklist of successors to be processed.
    """

    worklist: WorkList[Successor[ResultType]] = field(default_factory=WorkList)
    visited: dict[ir.Block, set[Successor[ResultType]]] = field(default_factory=dict)


AbstractFrameType = TypeVar("AbstractFrameType", bound=AbstractFrame)


@dataclass
class AbstractInterpreter(InterpreterABC[AbstractFrameType, ResultType], ABC):
    """Abstract interpreter for the IR.

    This is a base class for implementing abstract interpreters for the IR.
    It provides a framework for implementing abstract interpreters given a
    bounded lattice type.

    The abstract interpreter is a forward dataflow analysis that computes
    the abstract values for each SSA value in the IR. The abstract values
    are computed by evaluating the statements in the IR using the abstract
    lattice operations.

    The abstract interpreter is implemented as a worklist algorithm. The
    worklist contains the successors of the current block to be processed.
    The abstract interpreter processes each successor by evaluating the
    statements in the block and updating the abstract values in the frame.

    The abstract interpreter provides hooks for customizing the behavior of
    the interpreter.
    The [`prehook_succ`][kirin.interp.abstract.AbstractInterpreter.prehook_succ] and
    [`posthook_succ`][kirin.interp.abstract.AbstractInterpreter.posthook_succ] methods
    can be used to perform custom actions before and after processing a successor.
    """

    lattice: type[BoundedLattice[ResultType]] = field(init=False)
    """lattice type for the abstract interpreter.
    """

    def __init_subclass__(cls) -> None:
        if ABC in cls.__bases__:
            return super().__init_subclass__()

        if not hasattr(cls, "lattice"):
            raise TypeError(
                f"missing lattice attribute in abstract interpreter class {cls}"
            )
        cls.void = cls.lattice.bottom()
        cls.keys += ("abstract",)
        super().__init_subclass__()

    def recursion_limit_reached(self) -> ResultType:
        return self.lattice.bottom()

    # helper methods
    @overload
    @staticmethod
    def join_results(old: None, new: None) -> None: ...
    @overload
    @staticmethod
    def join_results(
        old: ReturnValue[ResultType], new: ReturnValue[ResultType]
    ) -> ReturnValue[ResultType]: ...
    @overload
    @staticmethod
    def join_results(
        old: YieldValue[ResultType], new: YieldValue[ResultType]
    ) -> YieldValue[ResultType]: ...
    @overload
    @staticmethod
    def join_results(
        old: tuple[ResultType], new: tuple[ResultType]
    ) -> tuple[ResultType]: ...
    @overload
    @staticmethod
    def join_results(
        old: AbsIntResultType[ResultType], new: AbsIntResultType[ResultType]
    ) -> AbsIntResultType[ResultType]: ...

    @staticmethod
    def join_results(
        old: AbsIntResultType[ResultType],
        new: AbsIntResultType[ResultType],
    ) -> AbsIntResultType[ResultType]:
        if old is None:
            return new
        elif new is None:
            return old

        if isinstance(old, ReturnValue) and isinstance(new, ReturnValue):
            return ReturnValue(old.value.join(new.value))
        elif isinstance(old, YieldValue) and isinstance(new, YieldValue):
            return YieldValue(
                tuple(
                    old_val.join(new_val)
                    for old_val, new_val in zip(old.values, new.values)
                )
            )
        elif isinstance(old, tuple) and isinstance(new, tuple):
            return tuple(old_val.join(new_val) for old_val, new_val in zip(old, new))
        else:
            return None

    T = TypeVar("T")

    @classmethod
    def maybe_const(cls, value: ir.SSAValue, type_: type[T]) -> T | None:
        """Get a constant value of a given type.

        If the value is not a constant or the constant is not of the given type, return
        `None`.
        """
        from kirin.analysis.const.lattice import Value

        hint = value.hints.get("const")
        if isinstance(hint, Value) and isinstance(hint.data, type_):
            return hint.data

    @classmethod
    def expect_const(cls, value: ir.SSAValue, type_: type[T]):
        """Expect a constant value of a given type.

        If the value is not a constant or the constant is not of the given type, raise
        an `InterpreterError`.
        """
        hint = cls.maybe_const(value, type_)
        if hint is None:
            raise InterpreterError(f"expected {type_}, got {hint}")
        return hint
