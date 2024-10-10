from kirin.codegen import DialectEmit, Printer, impl

from .dialect import dialect
from .stmts import Assert, Branch, ConditionalBranch


@dialect.register(key="print")
class Print(DialectEmit):

    @impl(Assert)
    def print_assert(self, printer: Printer, stmt: Assert):
        with printer.rich(style=printer.color.keyword):
            printer.print_name(stmt)

        printer.plain_print(" ")
        printer.print_SSAValue(stmt.condition)

        if stmt.message:
            printer.plain_print(", ")
            printer.print_SSAValue(stmt.message)

    @impl(Branch)
    def print_branch(self, printer: Printer, stmt: Branch):
        with printer.rich(style=printer.color.keyword):
            printer.print_name(stmt)

        printer.plain_print(" ")
        printer.plain_print(f"^{printer.state.block_id[stmt.successor]}")
        printer.print_seq(
            stmt.arguments,
            emit=printer.print_SSAValue,
            delim=", ",
            prefix="(",
            suffix=")",
        )

    @impl(ConditionalBranch)
    def print_conditional_branch(self, printer: Printer, stmt: ConditionalBranch):
        with printer.rich(style=printer.color.keyword):
            printer.print_name(stmt)

        printer.plain_print(" ")
        printer.print_SSAValue(stmt.cond)

        with printer.rich(style=printer.color.keyword):
            printer.plain_print(" goto ")

        printer.plain_print(f"^{printer.state.block_id[stmt.then_successor]}")
        printer.plain_print("(")
        printer.print_seq(stmt.then_arguments, emit=printer.print_SSAValue, delim=", ")
        printer.plain_print(")")

        with printer.rich(style=printer.color.keyword):
            printer.plain_print(" else ")

        printer.plain_print(f"^{printer.state.block_id[stmt.else_successor]}")
        printer.plain_print("(")
        printer.print_seq(stmt.else_arguments, emit=printer.print_SSAValue, delim=", ")
        printer.plain_print(")")
