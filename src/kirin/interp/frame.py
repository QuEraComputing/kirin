from abc import ABC, abstractmethod
from typing import Any, Generic, TypeVar, Iterable
from dataclasses import field, dataclass

from typing_extensions import Self

from kirin.ir import SSAValue, Statement

from .exceptions import InterpreterError

ValueType = TypeVar("ValueType")


@dataclass
class FrameABC(ABC, Generic[ValueType]):
    code: Statement
    """func statement being interpreted.
    """

    @classmethod
    @abstractmethod
    def from_func_like(cls, code: Statement) -> Self:
        """Create a new frame for the given method."""
        ...

    @abstractmethod
    def get(self, key: SSAValue) -> ValueType: ...

    @abstractmethod
    def set(self, key: SSAValue, value: ValueType) -> None: ...

    def get_values(self, keys: Iterable[SSAValue]) -> tuple[ValueType, ...]:
        """Get the values of the given `SSAValue` keys."""
        return tuple(self.get(key) for key in keys)

    def set_values(self, keys: Iterable[SSAValue], values: Iterable[ValueType]) -> None:
        """Set the values of the given `SSAValue` keys."""
        for key, value in zip(keys, values):
            self.set(key, value)

    @abstractmethod
    def set_stmt(self, stmt: Statement) -> Self:
        """Set the current statement."""
        ...


@dataclass
class Frame(FrameABC[ValueType]):
    lino: int = 0
    stmt: Statement | None = None
    """statement being interpreted.
    """

    globals: dict[str, Any] = field(default_factory=dict)
    """Global variables this frame has access to.
    """

    # NOTE: we are sharing the same frame within blocks
    # this is because we are validating e.g SSA value pointing
    # to other blocks separately. This avoids the need
    # to have a separate frame for each block.
    entries: dict[SSAValue, ValueType] = field(default_factory=dict)
    """SSA values and their corresponding values.
    """

    @classmethod
    def from_func_like(cls, code: Statement) -> Self:
        return cls(code=code)

    def get(self, key: SSAValue) -> ValueType:
        """Get the value for the given [`SSAValue`][kirin.ir.SSAValue].

        Args:
            key(SSAValue): The key to get the value for.

        Returns:
            ValueType: The value.

        Raises:
            InterpreterError: If the value is not found. This will be catched by the interpreter
                and will be converted to an [`interp.Err`][kirin.interp.Err] in the interpretation
                results.
        """
        err = InterpreterError(f"SSAValue {key} not found")
        value = self.entries.get(key, err)
        if isinstance(value, InterpreterError):
            raise err
        else:
            return value

    ExpectedType = TypeVar("ExpectedType")

    def get_typed(self, key: SSAValue, type_: type[ExpectedType]) -> ExpectedType:
        """Similar to [`get`][kirin.interp.frame.Frame.get] but also checks the type.

        Args:
            key(SSAValue): The key to get the value for.
            type_(type): The expected type.

        Returns:
            ExpectedType: The value.

        Raises:
            InterpreterError: If the value is not of the expected type. This will be catched
                by the interpreter and will be converted to an [`interp.Err`][kirin.interp.Err]
                in the interpretation results.
        """
        value = self.get(key)
        if not isinstance(value, type_):
            raise InterpreterError(f"expected {type_}, got {type(value)}")
        return value

    def set(self, key: SSAValue, value: ValueType) -> None:
        self.entries[key] = value

    def set_stmt(self, stmt: Statement) -> Self:
        self.stmt = stmt
        return self
