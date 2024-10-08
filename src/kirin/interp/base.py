from abc import ABC, abstractmethod
from collections.abc import Iterable
from dataclasses import dataclass, field
from enum import Enum
from typing import TYPE_CHECKING, ClassVar, Generic, TypeVar

from kirin.exceptions import InterpreterError
from kirin.interp.frame import Frame
from kirin.interp.state import InterpreterState
from kirin.interp.value import Err, NoReturn, Result, ResultValue
from kirin.ir import Dialect, DialectGroup, Region, Statement, traits
from kirin.ir.method import Method

if TYPE_CHECKING:
    from kirin.interp.impl import Signature, StatementImpl

ValueType = TypeVar("ValueType")


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
            return ResultValue(self.value)


@dataclass
class BaseInterpreter(ABC, Generic[ValueType]):
    """A base class for interpreters."""

    keys: ClassVar[list[str]]
    """The name of the interpreter to select from dialects by order.
    """
    dialects: DialectGroup
    """The dialects to interpret."""

    registry: dict["Signature", "StatementImpl"] = field(init=False, repr=False)
    """A mapping of statement signature to their implementation.
    """
    fallbacks: dict[Dialect, "StatementImpl"] = field(init=False, repr=False)
    state: InterpreterState = field(init=False, repr=False)
    """The interpreter state.
    """
    fuel: int | None = field(default=None, init=False, kw_only=True)
    """The fuel limit.
    """

    def __init__(
        self, dialects: DialectGroup | Iterable[Dialect], *, fuel: int | None = None
    ):
        if not isinstance(dialects, DialectGroup):
            dialects = DialectGroup(dialects)
        self.dialects = dialects

        self.registry, self.fallbacks = self.dialects.registry.interpreter(
            keys=self.keys
        )
        self.state = InterpreterState()
        self.fuel = fuel

    def eval(
        self,
        mt: Method,
        args: tuple[ValueType, ...],
        kwargs: dict[str, ValueType] | None = None,
    ) -> InterpResult[ValueType]:
        """Evaluate a method."""
        interface = mt.code.get_trait(traits.CallableStmtInterface)
        if interface is None:
            raise InterpreterError(f"compiled method {mt} is not callable")

        self.state.push_frame(Frame.from_method(mt))
        body = interface.get_callable_region(mt.code)
        # NOTE: #self# is not user input so it is not
        # in the args, +1 is for self
        args = self.get_args(mt.arg_names[len(args) + 1 :], args, kwargs)
        # NOTE: this should be checked via static validation, we just assume
        # number of args is correct here
        # NOTE: Method is used as if it is a singleton type, but it is not recognized by mypy
        results = self.run_method_region(mt, body, args)
        self.state.pop_frame()
        return results

    def run_method_region(
        self, mt: Method, body: Region, args: tuple[ValueType, ...]
    ) -> InterpResult[ValueType]:
        return self.run_ssacfg_region(body, (mt,) + args)  # type: ignore

    @staticmethod
    def get_args(left_arg_names, args: tuple, kwargs: dict | None) -> tuple:
        if kwargs:
            # NOTE: #self# is not user input so it is not
            # in the args, +1 is for self
            for name in left_arg_names:
                args += (kwargs[name],)
        return args

    def run_stmt(self, stmt: Statement, args: tuple) -> Result[ValueType]:
        "run a statement within the current frame"
        if self.state.frames:
            # NOTE: if run_stmt is called directly,
            # there is no frame being pushed, we only
            # push a frame when we call a method
            frame = self.state.current_frame()
            frame.stmt = stmt

        return self.eval_stmt(stmt, args)

    def eval_stmt(self, stmt: Statement, args: tuple) -> Result[ValueType]:
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
    ) -> InterpResult[ValueType]: ...

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
