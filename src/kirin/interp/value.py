from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Generic, Tuple, TypeVar

from kirin.interp.frame import Frame
from kirin.ir import Block, CallableStmtInterface, SymbolOpInterface

ValueType = TypeVar("ValueType")


@dataclass(init=False)
class Result(ABC, Generic[ValueType]):
    """Base class of interpretation results."""

    @abstractmethod
    def __len__(self) -> int: ...


@dataclass(init=False)
class NoReturn(Result[ValueType]):
    """No return value from a statement evaluation."""

    def __len__(self) -> int:
        return 0


@dataclass(init=False)
class ResultValue(Result[ValueType]):
    """Result values from a statement evaluation."""

    values: Tuple[ValueType, ...]

    def __init__(self, *values: ValueType):
        self.values = tuple(values)

    def __len__(self) -> int:
        return len(self.values)


@dataclass(init=False)
class ReturnValue(Result[ValueType]):
    """Return value from a statement evaluation."""

    result: ValueType

    def __init__(self, result: ValueType):
        super().__init__()
        self.result = result

    def __len__(self) -> int:
        return 0


@dataclass(init=False)
class Successor(Result[ValueType]):
    """Successor block from a statement evaluation."""

    block: Block
    block_args: Tuple[ValueType, ...]

    def __init__(self, block: Block, *block_args: ValueType):
        super().__init__()
        self.block = block
        self.block_args = block_args

    def __hash__(self) -> int:
        return hash(self.block)

    def __len__(self) -> int:
        return 0


@dataclass(init=False)
class Err(Result[ValueType]):
    """Error result from a statement evaluation."""

    exception: Exception
    frames: list[Frame]

    def __init__(self, exception: Exception, frames: list[Frame]):
        super().__init__()
        self.exception = exception
        self.frames = frames.copy()

    def __len__(self) -> int:
        return 0

    def __repr__(self) -> str:
        return f"Err({self.exception.__class__.__name__}: {self.exception})"

    def print_stack(self):
        """Print the stack trace of the error."""
        top_method_code = self.frames[0].method.code
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
            if frame.stmt:
                frame.stmt.print()
        print(f"{self.exception.__class__.__name__}: {self.exception}")
        print(
            "================================ Python Stacktrace ================================"
        )

    def panic(self):
        """Raise the error."""
        raise self.exception
