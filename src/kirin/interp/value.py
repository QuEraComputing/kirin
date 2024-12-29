from typing import Tuple, Generic, TypeVar, TypeAlias, final
from dataclasses import dataclass

from kirin.ir import Block, SymbolOpInterface, CallableStmtInterface
from kirin.interp.frame import Frame

ValueType = TypeVar("ValueType")


@dataclass(init=False)
class SpecialResult(Generic[ValueType]):
    pass


@final
@dataclass(init=False)
class ReturnValue(SpecialResult[ValueType]):
    """Return value from a statement evaluation."""

    result: ValueType

    def __init__(self, result: ValueType):
        super().__init__()
        self.result = result

    def __len__(self) -> int:
        return 0


@final
@dataclass(init=False)
class Successor(SpecialResult[ValueType]):
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


@final
@dataclass(init=False)
class Err(SpecialResult[ValueType]):
    """Error result from a statement evaluation."""

    exception: Exception
    frames: list[Frame]

    def __init__(self, exception: Exception, frames: list):
        super().__init__()
        self.exception = exception
        self.frames = frames.copy()

    def __len__(self) -> int:
        return 0

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
            if frame.stmt:
                frame.stmt.print()
        print(f"{self.exception.__class__.__name__}: {self.exception}")
        print(
            "================================ Python Stacktrace ================================"
        )

    def panic(self):
        """Raise the error."""
        raise self.exception


StatementResult: TypeAlias = tuple[ValueType, ...] | SpecialResult[ValueType]
MethodResult: TypeAlias = ValueType | Err[ValueType]
