from typing import TYPE_CHECKING, Type, Union, Generic, TypeVar, Callable, TypeAlias
from dataclasses import dataclass

from kirin.ir import Statement, types
from kirin.interp.value import Result

if TYPE_CHECKING:
    from kirin.interp.base import FrameABC, BaseInterpreter
    from kirin.interp.dialect import MethodTable

MethodTableSelf = TypeVar("MethodTableSelf", bound="MethodTable")
InterpreterType = TypeVar("InterpreterType", bound="BaseInterpreter")
FrameType = TypeVar("FrameType", bound="FrameABC")
StatementType = TypeVar("StatementType", bound=Statement)
MethodFunction: TypeAlias = Callable[
    [MethodTableSelf, InterpreterType, FrameType, StatementType], Result
]


@dataclass(frozen=True)
class Signature:
    stmt: Type[Statement]
    args: tuple[types.TypeAttribute, ...] | None = None

    def __repr__(self):
        if self.args:
            return f"{self.stmt.__name__}[{', '.join(map(repr, self.args))}]"
        else:
            return f"{self.stmt.__name__}[...]"


@dataclass
class ImplDef:
    parent: Type[Statement]
    signature: tuple[Signature, ...]
    impl: "MethodFunction"

    def __repr__(self):
        if self.parent.dialect:
            return f"interp {self.parent.dialect.name}.{self.parent.name}"
        else:
            return f"interp {self.parent.name}"


StatementType = TypeVar("StatementType", bound=Statement)


class impl(Generic[StatementType]):
    """Decorator to define an Interpreter implementation for a statement."""

    # TODO: validate only concrete types are allowed here

    def __init__(self, stmt: Type[StatementType], *args: types.TypeAttribute) -> None:
        self.stmt = stmt
        self.args = args

    def __call__(
        self,
        func: Union[
            Callable[
                [MethodTableSelf, InterpreterType, FrameType, StatementType], Result
            ],
            ImplDef,
        ],
    ) -> ImplDef:
        if self.args:
            sig = Signature(self.stmt, self.args)
        else:
            sig = Signature(self.stmt)

        if isinstance(func, ImplDef):
            return ImplDef(self.stmt, func.signature + (sig,), func.impl)
        else:
            return ImplDef(self.stmt, (sig,), func)
