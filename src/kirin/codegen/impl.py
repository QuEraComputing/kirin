# TODO: merge with impl in interp
from dataclasses import dataclass
from typing import TYPE_CHECKING, Callable, Generic, TypeVar, Union, overload

from kirin.ir import Attribute, Statement, TypeAttribute

if TYPE_CHECKING:
    from kirin.codegen.base import CodeGen
    from kirin.codegen.dialect import DialectEmit

Self = TypeVar("Self", bound="DialectEmit")
CodeGenType = TypeVar("CodeGenType", bound="CodeGen")
NodeType = TypeVar("NodeType", bound=Union[Statement, Attribute])
ResultType = TypeVar("ResultType")
ImplFunction = Callable[[Self, CodeGenType, NodeType], ResultType]
StatementImpl = Callable[[CodeGenType, NodeType], ResultType]
Signature = (
    type[Statement]
    | type[Attribute]
    | tuple[type[Statement], tuple[TypeAttribute, ...]]
)


@dataclass
class ImplDef(Generic[Self, CodeGenType, NodeType, ResultType]):
    parent: type[Statement]
    signature: tuple[Signature, ...]
    impl: ImplFunction[Self, CodeGenType, NodeType, ResultType]

    def __repr__(self):
        if self.parent.dialect:
            return f"emit {self.parent.dialect.name}.{self.parent.name}"
        else:
            return f"emit {self.parent.name}"


@dataclass
class AttributeEmitDef(Generic[Self, CodeGenType, NodeType, ResultType]):
    parent: type[Attribute]
    impl: ImplFunction[Self, CodeGenType, NodeType, ResultType]

    def __repr__(self) -> str:
        if self.parent.dialect:
            return f"emit {self.parent.dialect.name}.{self.parent.name}"
        else:
            return f"emit {self.parent.name}"


@dataclass
class MethodImpl(Generic[Self, CodeGenType, NodeType, ResultType]):
    parent: Self
    impl: ImplFunction[Self, CodeGenType, NodeType, ResultType]

    def __call__(self, interp: CodeGenType, stmt: NodeType):
        return self.impl(self.parent, interp, stmt)

    def __repr__(self) -> str:
        return f"emit impl `{self.impl.__name__}` in {repr(self.parent.__class__)}"


@overload
def impl(stmt: type[Statement], *args: TypeAttribute) -> Callable[
    [
        ImplFunction[Self, CodeGenType, NodeType, ResultType]
        | ImplDef[Self, CodeGenType, NodeType, ResultType]
    ],
    ImplDef[Self, CodeGenType, NodeType, ResultType],
]: ...


@overload
def impl(stmt: type[Attribute], *args: TypeAttribute) -> Callable[
    [ImplFunction[Self, CodeGenType, NodeType, ResultType]],
    AttributeEmitDef[Self, CodeGenType, NodeType, ResultType],
]: ...


def impl(
    stmt: type[Statement] | type[Attribute], *args: TypeAttribute
) -> (
    Callable[["ImplFunction"], AttributeEmitDef]
    | Callable[[Union["ImplFunction", ImplDef]], ImplDef]
):
    """Decorator to define an [`Codegen`][kirin.codegen.base.Codegen] implementation for a statement or attribute.

    Args:
        stmt (_type_): The statement or attribute to define the implementation for.

    """
    if issubclass(stmt, Attribute) and args:
        raise ValueError("Attributes do not have arguments")

    if issubclass(stmt, Attribute):

        def attribute_wrapper(
            func: Callable[["DialectEmit", "CodeGen", object], object]
        ):
            return AttributeEmitDef(stmt, func)

        return attribute_wrapper
    else:

        def statement_wrapper(func: Union["ImplFunction", ImplDef]):
            if args:
                sig = (stmt, args)
            else:
                sig = stmt

            if isinstance(func, ImplDef):
                return ImplDef(stmt, func.signature + (sig,), func.impl)
            else:
                return ImplDef(stmt, (sig,), func)

        return statement_wrapper
