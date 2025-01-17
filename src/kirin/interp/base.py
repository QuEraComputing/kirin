import sys
from abc import ABC, ABCMeta, abstractmethod
from enum import Enum
from typing import TYPE_CHECKING, Generic, TypeVar, Optional, Sequence
from dataclasses import dataclass
from collections.abc import Iterable

from typing_extensions import Self

from kirin.ir import Region, Dialect, Statement, DialectGroup, traits
from kirin.ir.method import Method
from kirin.exceptions import InterpreterError
from kirin.interp.impl import Signature
from kirin.interp.frame import FrameABC
from kirin.interp.state import InterpreterState
from kirin.interp.value import Err, MethodResult, StatementResult

if TYPE_CHECKING:
    from kirin.registry import StatementImpl

ValueType = TypeVar("ValueType")
FrameType = TypeVar("FrameType", bound=FrameABC)


@dataclass
class InterpResult(Generic[ValueType]):
    """This is used by the interpreter eval only."""

    value: MethodResult[ValueType]

    def expect(self) -> ValueType:
        if isinstance(self.value, Err):
            self.value.print_stack()
            return self.value.panic()
        return self.value

    def wrap_result(self) -> StatementResult[ValueType]:
        if isinstance(self.value, Err):
            return self.value
        return (self.value,)


class InterpreterMeta(ABCMeta):
    pass


class BaseInterpreter(ABC, Generic[FrameType, ValueType], metaclass=InterpreterMeta):
    """A base class for interpreters."""

    keys: list[str]
    """The name of the interpreter to select from dialects by order.
    """

    def __init__(
        self,
        dialects: DialectGroup | Iterable[Dialect],
        bottom: ValueType,
        *,
        fuel: int | None = None,
        max_depth: int = 128,
        max_python_recursion_depth: int = 8192,
    ):
        if not isinstance(dialects, DialectGroup):
            dialects = DialectGroup(dialects)
        self.dialects = dialects
        self.bottom = bottom

        self.registry = self.dialects.registry.interpreter(keys=self.keys)
        self.state: InterpreterState[FrameType] = InterpreterState()
        self.fuel = fuel
        self.max_depth = max_depth
        self.max_python_recursion_depth = max_python_recursion_depth

    def eval(
        self,
        mt: Method,
        args: tuple[ValueType, ...],
        kwargs: dict[str, ValueType] | None = None,
    ) -> InterpResult[ValueType]:
        """Evaluate a method."""
        current_recursion_limit = sys.getrecursionlimit()
        sys.setrecursionlimit(self.max_python_recursion_depth)
        args = self.get_args(mt.arg_names[len(args) + 1 :], args, kwargs)
        results = self.run_method(mt, args)
        sys.setrecursionlimit(current_recursion_limit)
        return InterpResult(results)

    @abstractmethod
    def run_method(
        self, method: Method, args: tuple[ValueType, ...]
    ) -> MethodResult[ValueType]:
        """How to run a method.

        This is defined by subclasses to describe what's the corresponding
        value of a method during the interpretation.

        Args:
            method (Method): the method to run.
            args (tuple[ValueType]): the arguments to the method, does not include self.

        Returns:
            ValueType: the result of the method.
        """
        ...

    def run_callable(
        self, code: Statement, args: tuple[ValueType, ...]
    ) -> MethodResult[ValueType]:
        """Run a callable statement.

        Args:
            code (Statement): the statement to run.
            args (tuple[ValueType]): the arguments to the statement,
                includes self if the corresponding callable region contains a self argument.

        Returns:
            ValueType: the result of the statement.
        """
        interface = code.get_trait(traits.CallableStmtInterface)
        if interface is None:
            raise InterpreterError(f"statement {code.name} is not callable")

        frame = self.new_frame(code)
        self.state.push_frame(frame)
        body = interface.get_callable_region(code)
        if not body.blocks:
            return self.finalize(self.state.pop_frame(), self.bottom)
        frame.set_values(body.blocks[0].args, args)
        results = self.run_callable_region(frame, code, body)
        return self.finalize(self.state.pop_frame(), results)

    def run_callable_region(
        self, frame: FrameType, code: Statement, region: Region
    ) -> MethodResult[ValueType]:
        """A hook defines how to run the callable region given
        the interpreter context.

        Note:
            This is experimental API, don't
            subclass it. The current reason of having it is mainly
            because we need to dispatch back to the MethodTable for
            emit.
        """
        return self.run_ssacfg_region(frame, region)

    @abstractmethod
    def new_frame(self, code: Statement) -> FrameType:
        """Create a new frame for the given method."""
        ...

    def finalize(
        self, frame: FrameType, results: MethodResult[ValueType]
    ) -> MethodResult[ValueType]:
        """Postprocess a frame after it is popped from the stack. This is
        called after a method is evaluated and the frame is popped.

        Note:
            Default implementation does nothing.
        """
        return results

    @staticmethod
    def get_args(
        left_arg_names, args: tuple[ValueType, ...], kwargs: dict[str, ValueType] | None
    ) -> tuple[ValueType, ...]:
        if kwargs:
            # NOTE: #self# is not user input so it is not
            # in the args, +1 is for self
            for name in left_arg_names:
                args += (kwargs[name],)
        return args

    @staticmethod
    def permute_values(
        arg_names: Sequence[str],
        values: tuple[ValueType, ...],
        kwarg_names: tuple[str, ...],
    ) -> tuple[ValueType, ...]:
        """Permute the arguments according to the method signature and
        the given keyword arguments, where the keyword argument names
        refer to the last n arguments in the values tuple.

        Args:
            arg_names: the argument names
            values: the values tuple (should not contain method itself)
            kwarg_names: the keyword argument names
        """
        n_total = len(values)
        if kwarg_names:
            kwargs = dict(zip(kwarg_names, values[n_total - len(kwarg_names) :]))
        else:
            kwargs = None

        positionals = values[: n_total - len(kwarg_names)]
        args = BaseInterpreter.get_args(
            arg_names[len(positionals) + 1 :], positionals, kwargs
        )
        return args

    def run_stmt(self, frame: FrameType, stmt: Statement) -> StatementResult[ValueType]:
        """Run a statement within the current frame. This is the entry
        point of running a statement. It will look up the statement implementation
        in the dialect registry, or optionally call a fallback implementation.

        Args:
            frame: the current frame
            stmt: the statement to run

        Returns:
            StatementResult: the result of running the statement

        Note:
            Overload this method for the following reasons:
            - to change the source tracking information
            - to take control of how to run a statement
            - to change the implementation lookup behavior that cannot acheive
                by overloading [`lookup_registry`][kirin.interp.base.BaseInterpreter.lookup_registry]

        Example:
            * implement an interpreter that only handles MyStmt:
            ```python
                class MyInterpreter(BaseInterpreter):
                    ...
                    def run_stmt(self, frame: FrameType, stmt: Statement) -> StatementResult[ValueType]:
                        if isinstance(stmt, MyStmt):
                            return self.run_my_stmt(frame, stmt)
                        else:
                            return ()
            ```

        """
        # TODO: update tracking information
        method = self.lookup_registry(frame, stmt)
        if method is not None:
            try:
                return method(self, frame, stmt)
            except InterpreterError as e:
                return Err(e, self.state.frames)

        return self.run_stmt_fallback(frame, stmt)

    def run_stmt_fallback(
        self, frame: FrameType, stmt: Statement
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
        # NOTE: not using f-string here because 3.10 and 3.11 have
        #  parser bug that doesn't allow f-string in raise statement
        raise ValueError(
            "no implementation for stmt "
            + stmt.print_str(end="")
            + " from "
            + str(type(self))
        )

    def build_signature(self, frame: FrameType, stmt: Statement) -> "Signature":
        """build signature for querying the statement implementation."""
        return Signature(stmt.__class__, tuple(arg.type for arg in stmt.args))

    def lookup_registry(
        self, frame: FrameType, stmt: Statement
    ) -> Optional["StatementImpl[Self, FrameType]"]:
        sig = self.build_signature(frame, stmt)
        if sig in self.registry.statements:
            return self.registry.statements[sig]
        elif (class_sig := Signature(stmt.__class__)) in self.registry.statements:
            return self.registry.statements[class_sig]
        return

    @abstractmethod
    def run_ssacfg_region(
        self, frame: FrameType, region: Region
    ) -> MethodResult[ValueType]: ...

    class FuelResult(Enum):
        Stop = 0
        Continue = 1

    def consume_fuel(self) -> FuelResult:
        if self.fuel is None:  # no fuel limit
            return self.FuelResult.Continue

        if self.fuel == 0:
            return self.FuelResult.Stop
        else:
            self.fuel -= 1
            return self.FuelResult.Continue
