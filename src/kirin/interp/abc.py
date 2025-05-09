from __future__ import annotations

import sys
from abc import ABC, abstractmethod
from typing import Any, Generic, TypeVar, ClassVar, Generator
from contextlib import contextmanager
from dataclasses import field, dataclass

from typing_extensions import Self

from kirin import ir
from kirin.exception import KIRIN_INTERP_STATE

from .frame import FrameABC
from .state import InterpreterState
from .table import Signature, BoundedDef
from .value import ReturnValue, RegionResult, SpecialValue, StatementResult
from .exceptions import InterpreterError, StackOverflowError

ValueType = TypeVar("ValueType")
FrameType = TypeVar("FrameType", bound=FrameABC)


@dataclass
class InterpreterABC(ABC, Generic[FrameType, ValueType]):
    keys: ClassVar[tuple[str, ...]]
    """The name of the interpreter to select from dialects by order.
    First matching key will be used.
    """

    void: ValueType = field(init=False)
    """What to return when the interpreter evaluates nothing.
    """

    dialects: ir.DialectGroup
    """The dialects this interpreter supports."""

    max_depth: int = field(default=800, kw_only=True)
    """The maximum depth of the interpreter stack."""
    max_python_recursion_depth: int = field(default=131072, kw_only=True)
    """The maximum recursion depth of the Python interpreter.
    """
    debug: bool = field(default=False, kw_only=True)
    """Enable debug mode."""

    registry: dict[Signature, BoundedDef] = field(init=False, compare=False)
    """The registry of implementations"""
    symbol_table: dict[str, ir.Statement] = field(init=False, compare=False)
    """The symbol table of the interpreter."""
    state: InterpreterState[FrameType] = field(init=False, compare=False)
    """The interpreter state."""
    __eval_lock: bool = field(default=False, init=False, repr=False)
    """Lock for the eval method."""

    def __init_subclass__(cls) -> None:
        super().__init_subclass__()
        if ABC in cls.__bases__:
            return

        if not hasattr(cls, "keys"):
            raise TypeError(f"keys is not defined for class {cls.__name__}")
        if not hasattr(cls, "void"):
            raise TypeError(f"void is not defined for class {cls.__name__}")

    def __post_init__(self) -> None:
        self.registry = self.dialects.registry.interpreter(keys=self.keys)
        self.symbol_table = self.dialects.symbol_table

    def initialize(self) -> Self:
        self.state = InterpreterState()
        return self

    @abstractmethod
    def initialize_frame(
        self, node: ir.Statement, *, has_parent_access: bool = False
    ) -> FrameType:
        """Initialize a new call frame for the given callable node."""
        ...

    def call(
        self, node: ir.Statement | ir.Method, *args: ValueType, **kwargs: ValueType
    ) -> tuple[FrameType, ValueType]:
        """Call a given callable node with the given arguments.

        This method is used to call a node that has a callable trait and a
        corresponding implementation of its callable region execution convention in
        the interpreter.

        Args:
            node: the callable node to call
            args: the arguments to pass to the callable node
            kwargs: the keyword arguments to pass to the callable node

        Returns:
            tuple[FrameType, ValueType]: the frame and the result of the call

        Raises:
            InterpreterError: if the interpreter is already evaluating
            StackOverflowError: if the maximum depth of the interpreter stack is reached
        """
        if isinstance(node, ir.Method):
            return self.__call_method(node, *args, **kwargs)

        with self.new_frame(node) as frame:
            return frame, self.frame_call(frame, node, *args, **kwargs)

    def eval(self, node: ir.Statement) -> tuple[FrameType, StatementResult[ValueType]]:
        with self.new_frame(node) as frame:
            return frame, self.frame_eval(frame, node)

    def __call_method(
        self, node: ir.Method, *args: ValueType, **kwargs: ValueType
    ) -> tuple[FrameType, ValueType]:
        if self.__eval_lock:
            raise InterpreterError(
                f"Interpreter {self.__class__.__name__} is already evaluating, "
                f"consider calling the bare `method.code` instead of the method"
            )

        if node.nargs != len(args) + len(kwargs):
            raise InterpreterError(
                f"Method {node} called with {len(args) + len(kwargs)} "
                f"arguments, expected {node.nargs}"
            )

        with self.eval_context():
            return self.call(node.code, *args, **kwargs)

    @contextmanager
    def eval_context(self):
        """Context manager to set the recursion limit and initialize the interpreter.

        This context manager sets the recursion limit to the maximum depth of
        the interpreter stack. It is used to prevent stack overflow when calling
        recursive functions.
        """
        if self.__eval_lock:
            raise InterpreterError(
                f"Interpreter {self.__class__.__name__} is already evaluating, "
                f"consider calling the bare `method.code` instead of the method"
                f" or use the bare `frame_call`/`frame_eval` methods"
            )

        self.__eval_lock = True
        self.initialize()
        current_recursion_limit = sys.getrecursionlimit()
        sys.setrecursionlimit(self.max_python_recursion_depth)
        try:
            yield self.max_python_recursion_depth
        except Exception as e:
            # NOTE: insert the interpreter state into the exception
            # so we can print the stack trace
            setattr(e, KIRIN_INTERP_STATE, self.state)
            raise e
        finally:
            self.__eval_lock = False
            sys.setrecursionlimit(current_recursion_limit)

    def frame_call(
        self,
        frame: FrameType,
        node: ir.Statement,
        *args: ValueType,
        **kwargs: ValueType,
    ) -> ValueType:
        """Call a given callable node with the given arguments in a given frame.

        This method is used to call a node that has a callable trait and a
        corresponding implementation of its callable region execution convention in
        the interpreter.
        """
        if entry := node.get_trait(ir.EntryPointInterface):
            node = self.symbol_table[entry.get_entry_point_symbol(node)]
        trait = node.get_present_trait(ir.CallableStmtInterface)
        args = trait.align_input_args(node, *args, **kwargs)
        region = trait.get_callable_region(node)
        if self.state.depth >= self.max_depth:
            return self.recursion_limit_reached()

        ret = self.frame_call_region(frame, node, region, *args)
        if isinstance(ret, ReturnValue):
            return ret.value
        elif not ret:  # empty result or None
            return self.void
        raise InterpreterError(
            f"callable region {node.name} does not return `ReturnValue`, got {ret}"
        )

    def recursion_limit_reached(self) -> ValueType:
        """Handle the recursion limit reached.

        This method is called when the maximum depth of the interpreter stack
        when calling a callable node is reached. By default a `StackOverflowError`
        is raised. Overload this method to provide a custom behavior, e.g. in
        the case of abstract interpreter, the recursion limit returns a bottom
        value.
        """
        raise StackOverflowError(
            f"Interpreter {self.__class__.__name__} stack "
            f"overflow at {self.state.depth}"
        )

    def frame_call_region(
        self,
        frame: FrameType,
        node: ir.Statement,
        region: ir.Region,
        *args: ValueType,
    ) -> RegionResult:
        """Call a given callable region with the given arguments in a given frame.

        This method is used to call a region that has a callable trait and a
        corresponding implementation of its callable region execution convention in
        the interpreter.

        Args:
            frame: the frame to call the region in
            node: the node to call the region on
            region: the region to call
            args: the arguments to pass to the region

        Returns:
            RegionResult: the result of the call

        Raises:
            InterpreterError: if cannot find a matching implementation for the region.
        """
        region_trait = node.get_present_trait(ir.RegionInterpretationTrait)
        how = self.registry.get(Signature(region_trait))
        if how is None:
            raise InterpreterError(
                f"Interpreter {self.__class__.__name__} does not "
                f"support {node} using {region_trait} convention"
            )
        region_trait.set_region_input(frame, region, *args)
        return how(self, frame, region)

    @contextmanager
    def new_frame(
        self, node: ir.Statement, *, has_parent_access: bool = False
    ) -> Generator[FrameType, Any, None]:
        """Create a new frame for the given node.

        This method is used to create a new call frame for the given node. The
        frame is pushed on the stack and popped when the context manager
        is exited. The frame is initialized with the given node and the
        given arguments.

        Args:
            node: the node to create the frame for
            has_parent_access: if the frame has access to the parent frame entries
                (default: False)

        Returns:
            Generator[FrameType, Any, None]: the frame
        """
        frame = self.initialize_frame(node, has_parent_access=has_parent_access)
        self.state.push_frame(frame)
        try:
            yield frame
        finally:
            self.state.pop_frame()

    def frame_eval(
        self, frame: FrameType, node: ir.Statement
    ) -> StatementResult[ValueType]:
        """Run a statement within the current frame. This is the entry
        point of running a statement. It will look up the statement implementation
        in the dialect registry, or optionally call a fallback implementation.

        Args:
            frame: the current frame
            node: the statement to run

        Returns:
            StatementResult: the result of running the statement
        """
        method = self.lookup_registry(frame, node)
        if method is not None:
            results = method(self, frame, node)
            if self.debug and not isinstance(results, (tuple, SpecialValue)):
                raise InterpreterError(
                    f"method must return tuple or SpecialResult, got {results}"
                )
            return results
        elif node.dialect not in self.dialects:
            name = node.dialect.name if node.dialect else "None"
            dialects = ", ".join(d.name for d in self.dialects)
            raise InterpreterError(
                f"Interpreter {self.__class__.__name__} does not "
                f"support {node} using {name} dialect. "
                f"Expected {dialects}"
            )

        return self.eval_fallback(frame, node)

    def eval_fallback(
        self, frame: FrameType, node: ir.Statement
    ) -> StatementResult[ValueType]:
        """The fallback implementation of statements.

        This is called when no implementation is found for the statement.

        Args:
            frame: the current frame
            stmt: the statement to run

        Returns:
            StatementResult: the result of running the statement

        Note:
            Overload this method to provide a fallback implementation for statements.
        """
        raise NotImplementedError(
            f"Missing implementation for {type(node).__name__} at {node.source}"
        )

    def lookup_registry(
        self, frame: FrameType, node: ir.Statement
    ) -> BoundedDef | None:
        sig = self.build_signature(frame, node)
        if sig in self.registry:
            return self.registry[sig]
        elif (method := self.registry.get(Signature(type(node)))) is not None:
            return method
        else:
            return None

    def build_signature(self, frame: FrameType, node: ir.Statement) -> Signature:
        return Signature(node.__class__, tuple(arg.type for arg in node.args))
