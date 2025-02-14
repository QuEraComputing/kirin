from typing import (
    TYPE_CHECKING,
    Any,
    Type,
    Union,
    Generic,
    TypeVar,
    Callable,
    TypeAlias,
    overload,
)
from dataclasses import dataclass

from kirin import types
from kirin.ir import Attribute, Statement
from kirin.interp.value import StatementResult

if TYPE_CHECKING:
    from kirin.interp.base import FrameABC, BaseInterpreter
    from kirin.interp.table import MethodTable

MethodTableSelf = TypeVar("MethodTableSelf", bound="MethodTable")
InterpreterType = TypeVar("InterpreterType", bound="BaseInterpreter")
FrameType = TypeVar("FrameType", bound="FrameABC")
StatementType = TypeVar("StatementType", bound=Statement)
AttributeType = TypeVar("AttributeType", bound=Attribute)
MethodFunction: TypeAlias = Callable[
    [MethodTableSelf, InterpreterType, FrameType, StatementType], StatementResult
]
AttributeFunction: TypeAlias = Callable[
    [MethodTableSelf, InterpreterType, AttributeType], Any
]


@dataclass(frozen=True)
class Signature:
    """Signature of a statement."""

    stmt: Type[Statement]
    args: tuple[types.TypeAttribute, ...] | None = None

    def __repr__(self):
        if self.args:
            return f"{self.stmt.__name__}[{', '.join(map(repr, self.args))}]"
        else:
            return f"{self.stmt.__name__}[...]"


SigType = TypeVar("SigType")
ImplType = TypeVar("ImplType")


@dataclass
class Def(Generic[SigType, ImplType]):
    """Base class of an interpreter implementation definition."""

    signature: SigType
    impl: ImplType


@dataclass
class ImplDef(Def[tuple[Signature, ...], "MethodFunction"]):
    """Definition of an interpreter implementation for a statement."""

    parent: Type[Statement]

    def __repr__(self):
        if self.parent.dialect:
            return f"interp {self.parent.dialect.name}.{self.parent.name}"
        else:
            return f"interp {self.parent.name}"

    def __call__(self, *args, **kwargs):
        return self.impl(*args, **kwargs)


@dataclass
class AttributeImplDef(Def[type[Attribute], "AttributeFunction"]):
    """Definition of an interpreter implementation for an attribute."""

    def __repr__(self):
        if self.signature.dialect:
            return f"attribute impl {self.signature.dialect.name}.{self.signature.name}"
        else:
            return f"attribute impl {self.signature.name}"


StatementType = TypeVar("StatementType", bound=Statement)
HeadType = TypeVar("HeadType")


class impl(Generic[HeadType]):
    """Decorator to define an interpreter implementation for a statement or attribute.

    !!! note
        While the `impl` decorator accepts both statements and attributes, and optionally
        statements with its type signature, unlike a programming language, the actual
        dispatch behavior given an instance of a statement or attribute is defined by the
        implementation of the interpreter
        (via [`lookup_registry`][kirin.interp.base.BaseInterpreter.lookup_registry]).

    # Example

    ```python
    @dialect.register
    class MyMethods(interp.MethodTable):
        @impl(Add)
        def interp_add(
            self,
            interp: Interpreter,
            frame: Frame,
            stmt: Add,
        ) -> StatementResult:
            ...
    ```
    """

    # TODO: validate only concrete types are allowed here

    def __init__(
        self, stmt_or_attribute: Type[HeadType], *args: types.TypeAttribute
    ) -> None:
        if args and issubclass(stmt_or_attribute, Attribute):
            raise ValueError("Attributes do not take arguments")
        self.stmt_or_attribute: type[HeadType] = stmt_or_attribute
        self.args = args

    @overload
    def __call__(
        self,
        func: Union[
            Callable[
                [MethodTableSelf, InterpreterType, FrameType, StatementType],
                StatementResult,
            ],
            ImplDef,
        ],
    ) -> ImplDef: ...

    @overload
    def __call__(
        self,
        func: Union[
            Callable[
                [MethodTableSelf, InterpreterType, AttributeType],
                Any,
            ],
            AttributeImplDef,
        ],
    ) -> AttributeImplDef: ...

    def __call__(
        self,
        func: Union[
            Callable[
                [MethodTableSelf, InterpreterType, FrameType, StatementType],
                StatementResult,
            ],
            Callable[
                [MethodTableSelf, InterpreterType, AttributeType],
                Any,
            ],
            ImplDef,
            AttributeImplDef,
        ],
    ) -> Def:
        if issubclass(self.stmt_or_attribute, Statement):
            return self._impl_statement(self.stmt_or_attribute, func)
        elif issubclass(self.stmt_or_attribute, Attribute):
            return self._impl_attribute(self.stmt_or_attribute, func)
        else:
            raise ValueError(f"Invalid statement type {self.stmt_or_attribute}")

    def _impl_attribute(
        self,
        attr: Type[Attribute],
        func: Union[Callable, Def],
    ) -> AttributeImplDef:
        if isinstance(func, Def):
            return AttributeImplDef(attr, func.impl)
        else:
            return AttributeImplDef(attr, func)

    def _impl_statement(
        self,
        stmt: Type[Statement],
        func: Union[Callable, Def],
    ) -> ImplDef:
        if self.args:
            sig = Signature(stmt, self.args)
        else:
            sig = Signature(stmt)

        if isinstance(func, Def):
            return ImplDef(func.signature + (sig,), func.impl, stmt)
        else:
            return ImplDef((sig,), func, stmt)
