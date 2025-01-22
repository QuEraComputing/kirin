from typing import TypeVar, Callable

from typing_extensions import Unpack, dataclass_transform

from kirin.ir import Statement
from kirin.decl import info
from kirin.decl.base import StatementOptions
from kirin.decl.verify import Verify
from kirin.decl.emit.init import EmitInit
from kirin.decl.emit.name import EmitName
from kirin.decl.emit.repr import EmitRepr
from kirin.decl.emit.traits import EmitTraits
from kirin.decl.emit.verify import EmitVerify
from kirin.decl.scan_fields import ScanFields
from kirin.decl.emit.dialect import EmitDialect
from kirin.decl.emit.property import EmitProperty
from kirin.decl.emit.typecheck import EmitTypeCheck


class StatementDecl(
    ScanFields,
    Verify,
    EmitInit,
    EmitProperty,
    EmitDialect,
    EmitName,
    EmitRepr,
    EmitTraits,
    EmitVerify,
    EmitTypeCheck,
):
    pass


StmtType = TypeVar("StmtType", bound=Statement)


@dataclass_transform(
    field_specifiers=(
        info.attribute,
        info.argument,
        info.region,
        info.result,
        info.block,
    )
)
def statement(
    cls=None,
    **kwargs: Unpack[StatementOptions],
) -> Callable[[type[StmtType]], type[StmtType]]:
    """Declare a new statement class.

    This decorator is used to declare a new statement class. It is used to
    generate the necessary boilerplate code for the class. The class should
    inherit from `kirin.ir.Statement`.

    Args:
        init(bool): Whether to generate an `__init__` method.
        repr(bool): Whether to generate a `__repr__` method.
        kw_only(bool): Whether to use keyword-only arguments in the `__init__`
            method.
        dialect(Optional[Dialect]): The dialect of the statement.
        property(bool): Whether to generate property methods for attributes.

    Example:
        The following is an example of how to use the `statement` decorator.

        ```python
        # optionally register the statement with
        # @statement(dialect=my_dialect_object)
        @statement
        class MyStatement(ir.Statement):
            name = "some_name"
            traits = frozenset({TraitA(), TraitB()})
            some_input: ir.SSAValue = info.argument()
            some_output: ir.ResultValue = info.result()
            body: ir.Region = info.region()
            successor: ir.Block = info.block()
        ```
    """

    def wrap(cls):
        decl = StatementDecl(cls, **kwargs)
        decl.scan_fields()
        decl.verify()
        decl.emit()
        decl.register()
        return cls

    if cls is None:
        return wrap
    return wrap(cls)


def fields(cls: type[Statement] | Statement) -> info.StatementFields:
    return getattr(cls, ScanFields._FIELDS)
