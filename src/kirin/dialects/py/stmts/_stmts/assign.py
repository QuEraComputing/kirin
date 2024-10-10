from kirin.decl import info, statement
from kirin.dialects.py import data, types
from kirin.dialects.py.stmts.dialect import dialect
from kirin.ir import Pure, ResultValue, SSAValue, Statement
from kirin.print.printer import Printer

T = types.PyTypeVar("T")


@statement(dialect=dialect)
class Alias(Statement):
    name = "alias"
    traits = frozenset({Pure()})
    value: SSAValue = info.argument(T)
    target: data.PyAttr[str] = info.attribute(property=True)
    result: ResultValue = info.result(T)

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self)
        printer.plain_print(" ")
        with printer.rich(style=printer.color.symbol):
            printer.plain_print(self.target.data)

        with printer.rich(style=printer.color.keyword):
            printer.plain_print(" = ")

        printer.print(self.value)
