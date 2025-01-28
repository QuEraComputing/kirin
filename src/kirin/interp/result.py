from typing import Generic, TypeVar
from dataclasses import dataclass

from kirin.ir.traits import SymbolOpInterface, CallableStmtInterface

from .frame import FrameABC
from .exceptions import WrapException

ValueType = TypeVar("ValueType")
FrameType = TypeVar("FrameType", bound=FrameABC)


@dataclass
class Result(Generic[ValueType]):
    """Result type for the interpreter.

    This is a generic result type that represents the result of interpretation.
    The result can be either an `Ok` value or an `Err` value. The `Ok` value
    represents a successful interpretation result, while the `Err` value
    represents an error during interpretation with a stack trace. One can use
    the `expect` method to extract the value from the result, which will raise
    an exception and print the stack trace if the result is an `Err`.
    """

    def expect(self) -> ValueType:
        raise NotImplementedError("unreachable")


@dataclass
class Err(Result[ValueType], Generic[FrameType, ValueType]):
    exception: Exception
    frames: list[FrameType]

    def __init__(self, exception: Exception, frames: list[FrameType]):
        super().__init__()
        self.exception = exception
        self.frames = frames

    def __repr__(self) -> str:
        return f"Err({self.exception.__class__.__name__}: {self.exception})"

    def print_stack(self):
        """Print the stack trace of the error."""
        top_method_code = self.frames[0].code
        if (call_trait := top_method_code.get_trait(CallableStmtInterface)) is None:
            raise ValueError(f"Method code {top_method_code} is not callable")

        region = call_trait.get_callable_region(top_method_code)
        name = (
            top_method_code.get_trait(SymbolOpInterface)
            .get_sym_name(top_method_code)  # type: ignore
            .data
        )
        args = ",".join(
            [
                (
                    f"{arg.name}"
                    if arg.type is arg.type.top()
                    else f"{arg.name}:{arg.type}"
                )
                for arg in region.blocks[0].args[1:]
            ]
        )
        print("Traceback (most recent call last):")
        print(f"  {name}({args})")
        for frame in reversed(self.frames):
            if frame.code:
                frame.code.print()
        print(f"{self.exception.__class__.__name__}: {self.exception}")
        print(
            "================================ Python Stacktrace ================================"
        )

    def expect(self) -> ValueType:
        self.print_stack()
        if isinstance(self.exception, WrapException):
            raise self.exception.exception from self.exception
        else:
            raise self.exception from None


@dataclass
class Ok(Result[ValueType]):
    value: ValueType

    def __len__(self) -> int:
        return 1

    def __repr__(self) -> str:
        return f"Ok({self.value})"

    def expect(self) -> ValueType:
        return self.value
