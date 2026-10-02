from __future__ import annotations

from abc import ABC
from typing import Generic, TypeVar, TypeAlias, overload
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
    """Store abstract SSA values and worklist state for an analysis call.

    In addition to the SSA-value entries inherited from :class:`Frame`, an
    abstract frame owns the pending control-flow successors and the visitation
    identities already evaluated for each block. A visitation identity combines
    a path-sensitive successor with the dependency generation maintained by the
    SSACFG solver. This lets an unchanged control-flow edge run again after an
    SSA value used by its destination changes.

    Assignments record values whose lattice meaning changed in ``_changes``.
    The SSACFG solver drains that set after evaluating a block and requeues any
    reached blocks that depend on those values. Multiple pending invalidations
    at the same latest generation therefore share one visitation identity.
    """

    worklist: WorkList[Successor[ResultType]] = field(default_factory=WorkList)
    visited: dict[ir.Block, set[tuple[Successor[ResultType], int]]] = field(
        default_factory=dict
    )
    _changes: set[ir.SSAValue] = field(
        default_factory=set, init=False, compare=False, repr=False
    )

    def set(self, key: ir.SSAValue, value: ResultType) -> None:
        previous = self.entries.get(key)
        self.entries[key] = value
        if previous is None or not (
            previous.is_subseteq(value) and value.is_subseteq(previous)
        ):
            self._changes.add(key)

    def _take_changes(self) -> set[ir.SSAValue]:
        changes = self._changes
        self._changes = set()
        return changes


AbstractFrameType = TypeVar("AbstractFrameType", bound=AbstractFrame)


@dataclass
class _RunningCall(Generic[ResultType]):
    """A call on the stack of an abstract interpreter."""

    node: ir.Statement
    args: tuple[ResultType, ...]
    assumed: ResultType
    """The result that a recursive call of `node` with `args` returns."""
    reached: bool = False
    """True if a recursive call of `node` with `args` returned `assumed`."""


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
    recursion_rounds: int = field(default=8, kw_only=True)
    """The rounds a call that reaches itself may take to settle before it is top."""
    _running: list[_RunningCall[ResultType]] = field(
        default_factory=list, init=False, repr=False
    )
    """The calls on the stack, innermost last."""

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

    def initialize(self):
        self._running = []
        return super().initialize()

    def call(
        self, node: ir.Statement | ir.Method, *args: ResultType, **kwargs: ResultType
    ) -> tuple[AbstractFrameType, ResultType]:
        """Call `node`, and solve a call that reaches itself as a fixpoint.

        A call of a callable node with the arguments of a call that is still on
        the stack returns the result assumed for that call, at first bottom. The
        outer call then runs again with its own result joined in, until the
        result stays within the assumption, which is then the result. A result
        that keeps growing, such as a tuple that nests deeper at every round,
        is assumed to be top after `recursion_rounds` rounds. This bounds the
        stack by the number of distinct calls.
        """
        if isinstance(node, ir.Method):
            return super().call(node, *args, **kwargs)
        trait = node.get_trait(ir.CallableStmtInterface)
        if trait is None:
            return super().call(node, *args, **kwargs)
        aligned = trait.align_input_args(node, *args, **kwargs)
        for running in self._running:
            if running.node is node and running.args == aligned:
                running.reached = True
                with self.new_frame(node) as frame:
                    return frame, running.assumed
        running = _RunningCall(node, aligned, self.lattice.bottom())
        self._running.append(running)
        try:
            for round in range(self.recursion_rounds + 1):
                frame, result = super().call(node, *args, **kwargs)
                if not running.reached:
                    return frame, result
                if result.is_subseteq(running.assumed):
                    return frame, running.assumed
                if round < self.recursion_rounds:
                    running.assumed = running.assumed.join(result)
                else:
                    running.assumed = self.lattice.top()
                running.reached = False
            frame, _ = super().call(node, *args, **kwargs)
            return frame, running.assumed
        finally:
            self._running.pop()

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
