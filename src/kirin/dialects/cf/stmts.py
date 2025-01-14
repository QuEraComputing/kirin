from kirin.ir import Block, SSAValue, Statement, IsTerminator
from kirin.decl import info, statement
from kirin.print.printer import Printer
from kirin.ir.attrs.types import Bool, String
from kirin.dialects.cf.dialect import dialect


@statement(dialect=dialect)
class Assert(Statement):
    name = "assert"
    traits = frozenset({})

    condition: SSAValue
    message: SSAValue = info.argument(String)

    def print_impl(self, printer: Printer) -> None:
        with printer.rich(style=printer.color.keyword):
            printer.print_name(self)

        printer.plain_print(" ")
        printer.print(self.condition)

        if self.message:
            printer.plain_print(", ")
            printer.print(self.message)


@statement(dialect=dialect)
class Branch(Statement):
    name = "br"
    traits = frozenset({IsTerminator()})

    arguments: tuple[SSAValue, ...]
    successor: Block = info.block()

    def print_impl(self, printer: Printer) -> None:
        with printer.rich(style=printer.color.keyword):
            printer.print_name(self)

        printer.plain_print(" ")
        printer.plain_print(printer.state.block_id[self.successor])
        printer.print_seq(
            self.arguments,
            delim=", ",
            prefix="(",
            suffix=")",
        )


@statement(dialect=dialect)
class ConditionalBranch(Statement):
    name = "cond_br"
    traits = frozenset({IsTerminator()})

    cond: SSAValue = info.argument(Bool)
    then_arguments: tuple[SSAValue, ...]
    else_arguments: tuple[SSAValue, ...]

    then_successor: Block = info.block()
    else_successor: Block = info.block()

    def print_impl(self, printer: Printer) -> None:
        with printer.rich(style=printer.color.keyword):
            printer.print_name(self)

        printer.plain_print(" ")
        printer.print(self.cond)

        with printer.rich(style=printer.color.keyword):
            printer.plain_print(" goto ")

        printer.plain_print(printer.state.block_id[self.then_successor])
        printer.plain_print("(")
        printer.print_seq(self.then_arguments, delim=", ")
        printer.plain_print(")")

        with printer.rich(style=printer.color.keyword):
            printer.plain_print(" else ")

        printer.plain_print(printer.state.block_id[self.else_successor])
        printer.plain_print("(")
        printer.print_seq(self.else_arguments, delim=", ")
        printer.plain_print(")")
