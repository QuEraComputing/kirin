from dataclasses import dataclass
from typing import TYPE_CHECKING, Callable, Type, TypeVar, Union

from kirin.interp.value import Result
from kirin.ir import Statement, TypeAttribute

if TYPE_CHECKING:
    from kirin.interp.base import BaseInterpreter
    from kirin.interp.dialect import DialectInterpreter

    Self = TypeVar("Self", bound="DialectInterpreter")
    InterpreterType = TypeVar("InterpreterType", bound="BaseInterpreter")
    StatementType = TypeVar("StatementType", bound=Statement)
    ImplFunction = Callable[[Self, InterpreterType, StatementType, tuple], Result]
    StatementImpl = Callable[[InterpreterType, StatementType, tuple], Result]
    Signature = Type[Statement] | tuple[Type[Statement], tuple[TypeAttribute, ...]]


@dataclass
class ImplDef:
    parent: Type[Statement]
    signature: tuple["Signature", ...]
    impl: "ImplFunction"

    def __repr__(self):
        if self.parent.dialect:
            return f"interp {self.parent.dialect.name}.{self.parent.name}"
        else:
            return f"interp {self.parent.name}"


@dataclass
class MethodImpl:
    parent: "DialectInterpreter"
    impl: "ImplFunction"

    def __call__(
        self, interp: "BaseInterpreter", stmt: Statement, values: tuple
    ) -> Result:
        return self.impl(self.parent, interp, stmt, values)

    def __repr__(self) -> str:
        return f"method impl `{self.impl.__name__}` in {repr(self.parent.__class__)}"


class impl:
    """Decorator to define an Interpreter implementation for a statement."""

    # TODO: validate only concrete types are allowed here

    def __init__(self, stmt: Type[Statement], *args: TypeAttribute) -> None:
        self.stmt = stmt
        self.args = args

    def __call__(self, func: Union["ImplFunction", ImplDef]) -> ImplDef:
        if self.args:
            sig = (self.stmt, self.args)
        else:
            sig = self.stmt

        if isinstance(func, ImplDef):
            return ImplDef(self.stmt, func.signature + (sig,), func.impl)
        else:
            return ImplDef(self.stmt, (sig,), func)
