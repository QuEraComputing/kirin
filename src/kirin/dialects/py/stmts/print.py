from kirin.codegen import DialectEmit, Printer, impl

from ._stmts import Alias, Constant
from .dialect import dialect


@dialect.register(key="print")
class Print(DialectEmit):

    @impl(Constant)
    def print_const(self, printer: Printer, stmt: Constant):
        printer.print_name(stmt)
        # printer.emit_Attribute(stmt.result.type)
        printer.plain_print("<{")
        printer.emit_Attribute(stmt.properties["value"])
        printer.plain_print("}>")

    @impl(Alias)
    def print_alias(self, printer: Printer, stmt: Alias):
        printer.print_name(stmt)
        printer.plain_print(" ")
        with printer.rich(style=printer.color.symbol):
            printer.plain_print(stmt.target.data)

        with printer.rich(style=printer.color.keyword):
            printer.plain_print(" = ")

        printer.print_SSAValue(stmt.value)
