from abc import ABC, abstractmethod
from typing import Generic, TypeVar, Iterable
from dataclasses import field, dataclass

from typing_extensions import Self

from kirin.ir import Block, SSAValue, Statement

from .undefined import Undefined, is_undefined
from .exceptions import InterpreterError

KeyType = TypeVar("KeyType")
ValueType = TypeVar("ValueType")


@dataclass
class FrameABC(ABC, Generic[KeyType, ValueType]):
    """Abstract base class for the IR interpreter's call frame.

    While the IR is in SSA form which does not have the need
    of scoping, the frame is still useful to keep track of the
    current statement being interpreted and the call stack. As
    well as various other interpreter state based on the specific
    interpreter implementation.

    This base class provides the minimum interface for the
    interpreter frame.
    """

    code: Statement
    """statement whose region is being interpreted, e.g a function.
    """
    parent: Self | None = field(default=None, kw_only=True, compare=True, repr=False)
    """Parent frame.
    """
    has_parent_access: bool = field(default=False, kw_only=True, compare=True)
    """If we have access to the entries of the parent frame."""

    lineno_offset: int = field(default=0, kw_only=True, compare=True)

    current_stmt: Statement | None = field(
        default=None, init=False, compare=False, repr=False
    )
    """Current statement being interpreted."""
    current_block: Block | None = field(
        default=None, init=False, compare=False, repr=False
    )
    """Current block being interpreted."""

    @abstractmethod
    def get(self, key: KeyType) -> ValueType:
        """Get the value for the given key.
        See also [`get_values`][kirin.interp.frame.Frame.get_values].

        Args:
            key(KeyType): The key to get the value for.

        Returns:
            ValueType: The value.
        """
        ...

    @abstractmethod
    def set(self, key: KeyType, value: ValueType) -> None:
        """Set the value for the given key.
        See also [`set_values`][kirin.interp.frame.Frame.set_values].

        Args:
            key(KeyType): The key to set the value for.
            value(ValueType): The value.
        """
        ...

    def get_values(self, keys: Iterable[KeyType]) -> tuple[ValueType, ...]:
        """Get the values of the given keys.
        See also [`get`][kirin.interp.frame.Frame.get].

        Args:
            keys(Iterable[KeyType]): The keys to get the values for.

        Returns:
            tuple[ValueType, ...]: The values.
        """
        return tuple(self.get(key) for key in keys)

    def set_values(self, keys: Iterable[KeyType], values: Iterable[ValueType]) -> None:
        """Set the values of the given keys.
        This is a convenience method to set multiple values at once.

        Args:
            keys(Iterable[KeyType]): The keys to set the values for.
            values(Iterable[ValueType]): The values.
        """
        for key, value in zip(keys, values, strict=True):
            self.set(key, value)


@dataclass
class Frame(FrameABC[SSAValue, ValueType]):
    entries: dict[SSAValue, ValueType] = field(default_factory=dict, kw_only=True)
    """SSA values and their corresponding values.
    """

    def get(self, key: SSAValue) -> ValueType:
        """Get the value for the given [`SSAValue`][kirin.ir.SSAValue].

        Args:
            key(SSAValue): The key to get the value for.

        Returns:
            ValueType: The value.

        Raises:
            InterpreterError: If the value is not found. This will be catched by the interpreter.
        """
        value = self.entries.get(key, Undefined)
        if is_undefined(value):
            if self.has_parent_access and self.parent:
                return self.parent.get(key)
            else:
                raise InterpreterError(f"SSAValue {key} not found")
        else:
            return value

    ExpectedType = TypeVar("ExpectedType")

    def get_casted(self, key: SSAValue, type_: type[ExpectedType]) -> ExpectedType:
        """Same as [`get`][kirin.interp.frame.Frame.get] except it
        forces the linter to think the value is of the expected type.

        Args:
            key(SSAValue): The key to get the value for.
            type_(type): The expected type.

        Returns:
            ExpectedType: The value.
        """
        return self.get(key)  # type: ignore

    def get_typed(self, key: SSAValue, type_: type[ExpectedType]) -> ExpectedType:
        """Similar to [`get`][kirin.interp.frame.Frame.get] but also checks the type.

        Args:
            key(SSAValue): The key to get the value for.
            type_(type): The expected type.

        Returns:
            ExpectedType: The value.

        Raises:
            InterpreterError: If the value is not of the expected type.
        """
        value = self.get(key)
        if not isinstance(value, type_):
            raise InterpreterError(f"expected {type_}, got {type(value)}")
        return value

    def set(self, key: SSAValue, value: ValueType) -> None:
        self.entries[key] = value
