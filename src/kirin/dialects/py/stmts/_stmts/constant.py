from typing import Generic, TypeVar

from kirin.decl import info, statement
from kirin.dialects.py import data, types
from kirin.dialects.py.stmts.dialect import dialect
from kirin.ir import ConstantLike, Pure, ResultValue, Statement
from kirin.print import Printer

T = TypeVar("T", covariant=True)


@statement(dialect=dialect)
class Constant(Statement, Generic[T]):
    name = "constant"
    traits = frozenset({Pure(), ConstantLike()})
    value: T = info.attribute(property=True)
    result: ResultValue = info.result()

    # NOTE: we allow py.Constant take data.PyAttr too
    def __init__(self, value: T | data.PyAttr[T]) -> None:
        if not isinstance(value, data.PyAttr):
            value = data.PyAttr(value)
        super().__init__(
            properties={"value": value},
            result_types=(types.PyConst(value.data, value.type),),
        )

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self)
        printer.plain_print(" ")
        printer.plain_print(repr(self.value))
        with printer.rich(style=printer.color.comment):
            printer.plain_print(" : ")
            printer.print(self.result.type)
