import sys
from abc import ABC, ABCMeta, abstractmethod
from enum import Enum
from typing import TYPE_CHECKING, Generic, TypeVar, ClassVar, Optional, Sequence
from dataclasses import field, dataclass

from typing_extensions import Self, deprecated

from kirin.ir import Block, Region, Statement, DialectGroup, traits
from kirin.ir.method import Method

from .impl import Signature
from .frame import FrameABC
from .state import InterpreterState
from .value import ReturnValue, SpecialValue, StatementResult
from .result import Ok, Err, Result
from .exceptions import InterpreterError

if TYPE_CHECKING:
    from kirin.registry import StatementImpl, InterpreterRegistry

ValueType = TypeVar("ValueType")
FrameType = TypeVar("FrameType", bound=FrameABC)


class InterpreterMeta(ABCMeta):
    """A metaclass for interpreters."""

    pass


@dataclass
class BaseInterpreter(ABC, Generic[FrameType, ValueType], metaclass=InterpreterMeta):
    """A base class for interpreters.

    This class defines the basic structure of an interpreter. It is
    designed to be subclassed to provide the actual implementation of
    the interpreter.

    ### Required Overrides
    When subclassing, if the subclass does not contain `ABC`,
    the subclass must define the following attributes:

    - `keys`: a list of strings that defines the order of dialects to select from.
    - `void`: the value to return when the interpreter evaluates nothing.
    """

    keys: ClassVar[list[str]]
    """The name of the interpreter to select from dialects by order.
    """
    void: ValueType = field(init=False)
    """What to return when the interpreter evaluates nothing.
    """
    dialects: DialectGroup
    """The dialects to interpret.
    """
    fuel: int | None = field(default=None, kw_only=True)
    """The fuel limit for the interpreter.
    """
    debug: bool = field(default=False, kw_only=True)
    """Whether to enable debug mode.
    """
    max_depth: int = field(default=128, kw_only=True)
    """The maximum depth of the interpreter stack.
    """
    max_python_recursion_depth: int = field(default=8192, kw_only=True)
    """The maximum recursion depth of the Python interpreter.
    """

    # global states
    registry: "InterpreterRegistry" = field(init=False, compare=False)
    """The interpreter registry.
    """
    symbol_table: dict[str, Statement] = field(init=False, compare=False)
    """The symbol table.
    """
    state: InterpreterState[FrameType] = field(init=False, compare=False)
    """The interpreter state.
    """

    # private
    _eval_lock: bool = field(default=False, init=False, compare=False)

    def __post_init__(self) -> None:
        self.registry = self.dialects.registry.interpreter(keys=self.keys)

    def initialize(self) -> Self:
        """Initialize the interpreter global states. This method is called right upon
        calling [`run`][kirin.interp.base.BaseInterpreter.run] to initialize the
        interpreter global states.

        !!! note "Default Implementation"
            This method provides default behavior but may be overridden by subclasses
            to customize or extend functionality.
        """
        self.symbol_table: dict[str, Statement] = {}
        self.state: InterpreterState[FrameType] = InterpreterState()
        return self

    def __init_subclass__(cls) -> None:
        super().__init_subclass__()
        if ABC in cls.__bases__:
            return

        if not hasattr(cls, "keys"):
            raise TypeError(f"keys is not defined for class {cls.__name__}")
        if not hasattr(cls, "void"):
            raise TypeError(f"void is not defined for class {cls.__name__}")

    @deprecated("use run instead")
    def eval(
        self,
        mt: Method,
        args: tuple[ValueType, ...],
        kwargs: dict[str, ValueType] | None = None,
    ) -> Result[ValueType]:
        return self.run(mt, args, kwargs)

    def run(
        self,
        mt: Method,
        args: tuple[ValueType, ...],
        kwargs: dict[str, ValueType] | None = None,
    ) -> Result[ValueType]:
        """Run a method. This is the main entry point of the interpreter.

        Args:
            mt (Method): the method to run.
            args (tuple[ValueType]): the arguments to the method, does not include self.
            kwargs (dict[str, ValueType], optional): the keyword arguments to the method.

        Returns:
            Result[ValueType]: the result of the method.
        """
        if self._eval_lock:
            raise InterpreterError(
                "recursive eval is not allowed, use run_method instead"
            )

        self._eval_lock = True
        self.initialize()
        current_recursion_limit = sys.getrecursionlimit()
        sys.setrecursionlimit(self.max_python_recursion_depth)
        args = self.get_args(mt.arg_names[len(args) + 1 :], args, kwargs)
        try:
            _, results = self.run_method(mt, args)
        except InterpreterError as e:
            # NOTE: initialize will create new State
            # so we don't need to copy the frames.
            return Err(e, self.state.frames)
        finally:
            self._eval_lock = False
            sys.setrecursionlimit(current_recursion_limit)
        return Ok(results)

    def run_stmt(
        self, stmt: Statement, args: tuple[ValueType, ...]
    ) -> StatementResult[ValueType]:
        """execute a statement with arguments in a new frame.

        Args:
            stmt (Statement): the statement to run.
            args (tuple[ValueType]): the arguments to the statement.

        Returns:
            StatementResult[ValueType]: the result of the statement.
        """
        frame = self.new_frame(stmt)
        self.state.push_frame(frame)
        frame.set_values(stmt.args, args)
        results = self.eval_stmt(frame, stmt)
        self.state.pop_frame()
        return results

    @abstractmethod
    def run_method(
        self, method: Method, args: tuple[ValueType, ...]
    ) -> tuple[FrameType, ValueType]:
        """How to run a method.

        This is defined by subclasses to describe what's the corresponding
        value of a method during the interpretation. Usually, this method
        just calls [`run_callable`][kirin.interp.base.BaseInterpreter.run_callable].

        Args:
            method (Method): the method to run.
            args (tuple[ValueType]): the arguments to the method, does not include self.

        Returns:
            ValueType: the result of the method.
        """
        ...

    def run_callable(
        self, code: Statement, args: tuple[ValueType, ...]
    ) -> tuple[FrameType, ValueType]:
        """Run a callable statement.

        Args:
            code (Statement): the statement to run.
            args (tuple[ValueType]): the arguments to the statement,
                includes self if the corresponding callable region contains a self argument.

        Returns:
            ValueType: the result of the statement.
        """
        if len(self.state.frames) >= self.max_depth:
            return self.eval_recursion_limit(self.state.current_frame())

        interface = code.get_trait(traits.CallableStmtInterface)
        if interface is None:
            raise InterpreterError(f"statement {code.name} is not callable")

        frame = self.new_frame(code)
        self.state.push_frame(frame)
        body = interface.get_callable_region(code)
        if not body.blocks:
            return self.state.pop_frame(), self.void
        frame.set_values(body.blocks[0].args, args)
        results = self.run_callable_region(frame, code, body)
        return self.state.pop_frame(), results

    def run_callable_region(
        self, frame: FrameType, code: Statement, region: Region
    ) -> ValueType:
        """A hook defines how to run the callable region given
        the interpreter context. Frame should be pushed before calling
        this method and popped after calling this method.

        A callable region is a region that can be called as a function.
        Unlike a general region (or the MLIR convention), it always return a value
        to be compatible with the Python convention.
        """
        results = self.run_ssacfg_region(frame, region)
        if isinstance(results, ReturnValue):
            return results.value
        elif not results:  # empty result or None
            return self.void
        raise InterpreterError(
            f"callable region {code.name} does not return `ReturnValue`, got {results}"
        )

    def run_block(self, frame: FrameType, block: Block) -> SpecialValue[ValueType]:
        """Run a block within the current frame.

        Args:
            frame: the current frame.
            block: the block to run.

        Returns:
            SpecialValue: the result of running the block terminator.
        """
        ...

    @abstractmethod
    def new_frame(self, code: Statement) -> FrameType:
        """Create a new frame for the given method."""
        ...

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

    def eval_stmt(
        self, frame: FrameType, stmt: Statement
    ) -> StatementResult[ValueType]:
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
                    def eval_stmt(self, frame: FrameType, stmt: Statement) -> StatementResult[ValueType]:
                        if isinstance(stmt, MyStmt):
                            return self.run_my_stmt(frame, stmt)
                        else:
                            return ()
            ```

        """
        # TODO: update tracking information
        method = self.lookup_registry(frame, stmt)
        if method is not None:
            results = method(self, frame, stmt)
            if self.debug and not isinstance(results, (tuple, SpecialValue)):
                raise InterpreterError(
                    f"method must return tuple or SpecialResult, got {results}"
                )
            return results
        elif stmt.dialect not in self.dialects:
            # NOTE: we should terminate the interpreter because this is a
            # deveoper error, not a user error.
            name = stmt.dialect.name if stmt.dialect else "None"
            raise ValueError(f"dialect {name} is not supported by {self.dialects}")

        return self.eval_stmt_fallback(frame, stmt)

    @deprecated("use eval_stmt_fallback instead")
    def run_stmt_fallback(
        self, frame: FrameType, stmt: Statement
    ) -> StatementResult[ValueType]:
        return self.eval_stmt_fallback(frame, stmt)

    def eval_stmt_fallback(
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
        raise InterpreterError(
            "no implementation for stmt "
            + stmt.print_str(end="")
            + " from "
            + str(type(self))
        )

    def eval_recursion_limit(self, frame: FrameType) -> tuple[FrameType, ValueType]:
        """Return the value of recursion exception, e.g in concrete
        interpreter, it will raise an exception if the limit is reached;
        in type inference, it will return a special value.
        """
        raise InterpreterError("maximum recursion depth exceeded")

    def build_signature(self, frame: FrameType, stmt: Statement) -> "Signature":
        """build signature for querying the statement implementation."""
        return Signature(stmt.__class__, tuple(arg.type for arg in stmt.args))

    def lookup_registry(
        self, frame: FrameType, stmt: Statement
    ) -> Optional["StatementImpl[Self, FrameType]"]:
        """Lookup the statement implementation in the registry.

        Args:
            frame: the current frame
            stmt: the statement to run

        Returns:
            Optional[StatementImpl]: the statement implementation if found, None otherwise.
        """
        sig = self.build_signature(frame, stmt)
        if sig in self.registry.statements:
            return self.registry.statements[sig]
        elif (class_sig := Signature(stmt.__class__)) in self.registry.statements:
            return self.registry.statements[class_sig]
        return

    @abstractmethod
    def run_ssacfg_region(
        self, frame: FrameType, region: Region
    ) -> tuple[ValueType, ...] | None | ReturnValue[ValueType]:
        """This implements how to run a region with MLIR SSA CFG convention.

        Args:
            frame: the current frame.
            region: the region to run.

        Returns:
            tuple[ValueType, ...] | SpecialValue[ValueType]: the result of running the region.

        when region returns `tuple[ValueType, ...]`, it means the region terminates normally
        with `YieldValue`. When region returns `ReturnValue`, it means the region terminates
        and needs to pop the frame. Region cannot return `Successor` because reference to
        external region is not allowed.
        """
        ...

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
