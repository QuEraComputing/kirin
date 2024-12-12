import sys
from abc import ABC, ABCMeta, abstractmethod
from enum import Enum
from typing import TYPE_CHECKING, Generic, TypeVar, Sequence
from dataclasses import dataclass
from collections.abc import Iterable

from kirin.ir import Region, Dialect, Statement, DialectGroup, traits
from kirin.ir.method import Method
from kirin.exceptions import InterpreterError
from kirin.interp.frame import FrameABC
from kirin.interp.state import InterpreterState
from kirin.interp.value import Err, Result, NoReturn

if TYPE_CHECKING:
    from kirin.interp.impl import Signature

ValueType = TypeVar("ValueType")
FrameType = TypeVar("FrameType", bound=FrameABC)


@dataclass(init=False)
class InterpResult(Generic[ValueType]):
    """This is used by the interpreter eval only."""

    value: ValueType | NoReturn
    err: Err[ValueType] | None = None

    def __init__(self, result: ValueType | NoReturn | Err):
        if isinstance(result, Err):
            self.err = result
            self.value = NoReturn()
        else:
            self.value = result

    def expect(self) -> ValueType:
        if self.err is not None:
            self.err.print_stack()
            return self.err.panic()
        elif isinstance(self.value, NoReturn):
            raise InterpreterError("no return value")
        else:
            return self.value

    def to_result(self) -> Result[ValueType]:
        if self.err is not None:
            return self.err
        elif isinstance(self.value, NoReturn):
            return NoReturn()
        else:
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
        *,
        fuel: int | None = None,
        max_depth: int = 128,
        max_python_recursion_depth: int = 8192,
    ):
        if not isinstance(dialects, DialectGroup):
            dialects = DialectGroup(dialects)
        self.dialects = dialects

        self.registry, self.fallbacks = self.dialects.registry.interpreter(
            keys=self.keys
        )
        self.state: InterpreterState[FrameType] = InterpreterState()
        self.fuel = fuel
        self.max_depth = max_depth
        self.max_python_recursion_depth = max_python_recursion_depth

    @abstractmethod
    def new_method_frame(self, mt: Method) -> FrameType:
        """Create a new frame for the given method."""
        ...

    def eval(
        self,
        mt: Method,
        args: tuple[ValueType, ...],
        kwargs: dict[str, ValueType] | None = None,
    ) -> InterpResult[ValueType]:
        """Evaluate a method."""
        current_recursion_limit = sys.getrecursionlimit()
        sys.setrecursionlimit(self.max_python_recursion_depth)
        interface = mt.code.get_trait(traits.CallableStmtInterface)
        if interface is None:
            raise InterpreterError(f"compiled method {mt} is not callable")

        if len(self.state.frames) >= self.max_depth:
            raise InterpreterError("maximum recursion depth exceeded")

        self.state.push_frame(self.new_method_frame(mt))
        body = interface.get_callable_region(mt.code)
        # NOTE: #self# is not user input so it is not
        # in the args, +1 is for self
        args = self.get_args(mt.arg_names[len(args) + 1 :], args, kwargs)
        # NOTE: this should be checked via static validation, we just assume
        # number of args is correct here
        # NOTE: Method is used as if it is a singleton type, but it is not recognized by mypy
        results = self.run_method_region(mt, body, args)
        self.postprocess_frame(self.state.pop_frame())
        sys.setrecursionlimit(current_recursion_limit)
        return InterpResult(results)

    @abstractmethod
    def run_method_region(
        self, mt: Method, body: Region, args: tuple[ValueType, ...]
    ) -> ValueType: ...

    def postprocess_frame(self, frame: FrameType) -> None:
        """Postprocess a frame after it is popped from the stack. This is
        called after a method is evaluated and the frame is popped. Default
        implementation does nothing.
        """
        return

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

        Args

        mt: the method
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

    def run_stmt(self, stmt: Statement, args: tuple) -> Result[ValueType]:
        "run a statement within the current frame"
        if self.state.frames:
            # NOTE: if run_stmt is called directly,
            # there is no frame being pushed, we only
            # push a frame when we call a method
            self.state.current_frame().set_stmt(stmt)
        return self.eval_stmt(stmt, args)

    def eval_stmt(
        self, stmt: Statement, args: tuple[ValueType, ...]
    ) -> Result[ValueType]:
        "simply evaluate a statement"
        sig = self.build_signature(stmt, args)
        if sig in self.registry:
            return self.registry[sig](self, stmt, args)
        elif stmt.__class__ in self.registry:
            return self.registry[stmt.__class__](self, stmt, args)
        elif stmt.dialect:
            return self.fallbacks[stmt.dialect](self, stmt, args)
        raise ValueError(f"no dialect for stmt {stmt}")

    def build_signature(self, stmt: Statement, args: tuple) -> "Signature":
        """build signature for querying the statement implementation."""
        return (stmt.__class__, tuple(arg.type for arg in stmt.args))

    @abstractmethod
    def run_ssacfg_region(
        self, region: Region, args: tuple[ValueType, ...]
    ) -> ValueType: ...

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
