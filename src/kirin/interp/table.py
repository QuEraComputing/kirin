from __future__ import annotations

from abc import ABC
from typing import TYPE_CHECKING, Generic, TypeVar, Callable, TypeAlias, overload
from dataclasses import dataclass

from kirin import ir, types

if TYPE_CHECKING:
    from .abc import InterpreterABC
    from .frame import FrameABC
    from .value import RegionResult, StatementResult

MethodTableType = TypeVar("MethodTableType", bound="MethodTable")
InterpreterType = TypeVar("InterpreterType", bound="InterpreterABC")
FrameType = TypeVar("FrameType", bound="FrameABC")
ValueType = TypeVar("ValueType")
Head = TypeVar("Head")
Ret = TypeVar("Ret")
ClassMethod: TypeAlias = Callable[
    [
        MethodTableType,
        InterpreterType,
        FrameType,
        Head,
    ],
    Ret,
]


@dataclass(frozen=True)
class Signature:
    head: type | ir.RegionInterpretationTrait
    args: tuple = ()


NodeType = TypeVar("NodeType")


@dataclass
class Def(Generic[MethodTableType, InterpreterType, FrameType, NodeType, Ret]):
    signature: tuple[Signature, ...]
    method: ClassMethod[MethodTableType, InterpreterType, FrameType, NodeType, Ret]

    def __init__(
        self,
        signature: Signature,
        func: (
            Def[MethodTableType, InterpreterType, FrameType, NodeType, Ret]
            | ClassMethod[MethodTableType, InterpreterType, FrameType, NodeType, Ret]
        ),
    ):
        if isinstance(func, Def):
            assert (
                signature.args is not None
            ), f"Signature with no parameters {signature} cannot have multiple implementations"
            self.signature = func.signature + (signature,)
            self.method = func.method
        else:
            self.signature = (signature,)
            self.method = func

        self.__name__ = func.__name__
        self.__doc__ = func.__doc__

    def __get__(
        self,
        method_table: MethodTableType | None,
        owner: type[MethodTableType] | None = None,
    ):
        if method_table is None:
            raise TypeError(
                f"cannot use impl decorator on {self.method}, only on MethodTable"
            )

        if owner is None:
            owner = type(method_table)

        if not issubclass(owner, MethodTable):
            raise TypeError(
                f"cannot use impl decorator on {owner}, only on MethodTable"
            )
        return BoundedDef(method_table, self.signature, self.method)


@dataclass
class BoundedDef(Generic[MethodTableType, InterpreterType, FrameType, NodeType, Ret]):
    parent: MethodTableType
    signature: tuple[Signature, ...]
    method: ClassMethod[MethodTableType, InterpreterType, FrameType, NodeType, Ret]

    def __call__(
        self,
        interpreter: InterpreterType,
        frame: FrameType,
        node: NodeType,
    ) -> Ret:
        return self.method(self.parent, interpreter, frame, node)

    def __repr__(self) -> str:
        return f"impl {self.method.__name__} in {repr(self.parent.__class__)}"


@overload
def impl(head: ir.RegionInterpretationTrait) -> Callable[
    [
        ClassMethod[
            MethodTableType,
            InterpreterType,
            FrameType,
            ir.Region,
            RegionResult[ValueType],
        ]
        | Def[
            MethodTableType,
            InterpreterType,
            FrameType,
            ir.Region,
            RegionResult[ValueType],
        ]
    ],
    Def[
        MethodTableType, InterpreterType, FrameType, ir.Region, RegionResult[ValueType]
    ],
]: ...


AttributeType = TypeVar("AttributeType", bound=ir.Attribute)


@overload
def impl(head: type[AttributeType]) -> Callable[
    [
        ClassMethod[
            MethodTableType,
            InterpreterType,
            FrameType,
            AttributeType,
            ValueType,
        ]
        | Def[
            MethodTableType,
            InterpreterType,
            FrameType,
            AttributeType,
            ValueType,
        ]
    ],
    Def[
        MethodTableType,
        InterpreterType,
        FrameType,
        AttributeType,
        ValueType,
    ],
]: ...


StatementType = TypeVar("StatementType", bound=ir.Statement)
MethodStatementType = TypeVar("MethodStatementType", bound=ir.Statement)


@overload
def impl(head: type[StatementType], *params: types.TypeAttribute) -> Callable[
    [
        ClassMethod[
            MethodTableType,
            InterpreterType,
            FrameType,
            MethodStatementType,
            StatementResult[ValueType],
        ]
        | Def[
            MethodTableType,
            InterpreterType,
            FrameType,
            MethodStatementType,
            StatementResult[ValueType],
        ]
    ],
    Def[
        MethodTableType,
        InterpreterType,
        FrameType,
        MethodStatementType,
        StatementResult[ValueType],
    ],
]: ...


def impl(head, *params):
    def wrapper(func: ClassMethod, /):
        return Def(Signature(head, params), func)

    if isinstance(head, ir.RegionInterpretationTrait) or issubclass(
        head, (ir.Statement, ir.Attribute)
    ):
        return wrapper
    else:
        raise TypeError(f"Unsupported type: {head}")


class MethodTable(ABC):
    """Base class to define lookup tables for interpreting code for IR nodes in a dialect."""

    pass
