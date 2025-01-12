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
from kirin.decl.emit.from_python_call import EmitFromPythonCall


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
    EmitFromPythonCall,
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


def fields(cls: type[Statement]) -> info.StatementFields:
    return getattr(cls, ScanFields._FIELDS)
