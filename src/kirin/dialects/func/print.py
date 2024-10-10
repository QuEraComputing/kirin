from kirin.codegen import DialectEmit, Printer, impl

from .dialect import dialect
from .stmts import Call, Function, GetField, Lambda, Return


@dialect.register(key="print")
class Print(DialectEmit):

    @impl(Call)
    def print_call(self, printer: Printer, stmt: Call):
        with printer.rich(style="red"):
            printer.print_name(stmt)
        printer.plain_print(" ")

        n_total = len(stmt.args)
        callee = stmt.args[0]
        printer.print_SSAValue(callee)

        positional = stmt.args[1 : n_total - len(stmt.kwargs.data)]
        kwargs = dict(
            zip(stmt.kwargs.data, stmt.args[n_total - len(stmt.kwargs.data) :])
        )

        printer.plain_print("(")
        printer.print_seq(positional, emit=printer.print_SSAValue)
        if kwargs:
            printer.plain_print(", ")
        printer.print_mapping(kwargs, emit=printer.print_SSAValue, delim=", ")
        printer.plain_print(")")

        with printer.rich(style="black"):
            printer.plain_print(" -> ")
            printer.print_seq(
                [result.type for result in stmt._results],
                emit=printer.emit_Attribute,
                delim=", ",
            )

    @impl(GetField)
    def print_getfield(self, printer: Printer, stmt: GetField):
        printer.print_name(stmt)
        printer.plain_print(
            "(", printer.state.ssa_id.get_name(stmt.obj), ", ", str(stmt.field), ")"
        )
        with printer.rich(style="black"):
            printer.plain_print(" -> ")
            printer.emit_Attribute(stmt.result.type)

    @impl(Return)
    def print_return(self, printer: Printer, stmt: Return):
        with printer.rich(style=printer.color.keyword):
            printer.print_name(stmt)

        if stmt.args:
            printer.plain_print(" ")
            printer.print_seq(stmt.args, emit=printer.print_SSAValue, delim=", ")

    @impl(Lambda)
    def print_lambda(self, printer: Printer, stmt: Lambda):
        with printer.rich(style=printer.color.keyword):
            printer.print_name(stmt)
        printer.plain_print(" ")

        with printer.rich(style=printer.color.symbol):
            printer.plain_print(stmt.sym_name)

        printer.print_seq(
            stmt.args, emit=printer.print_SSAValue, prefix="(", suffix=")", delim=", "
        )

        with printer.rich(style="bright_black"):
            printer.plain_print(" -> ")
            printer.emit_Attribute(stmt.signature.output)

        printer.plain_print(" ")
        printer.emit_Region(stmt.body)

        with printer.rich(style="black"):
            printer.plain_print(f" // func.lambda {stmt.sym_name}")

    @impl(Function)
    def print_fn(self, printer: Printer, stmt: Function):
        with printer.rich(style=printer.color.keyword):
            printer.print_name(stmt)
            printer.plain_print(" ")

        with printer.rich(style=printer.color.symbol):
            printer.plain_print(stmt.sym_name)

        printer.print_seq(stmt.signature.inputs, prefix="(", suffix=")", delim=", ")

        with printer.rich(style=printer.color.comment):
            printer.plain_print(" -> ")
            printer.emit_Attribute(stmt.signature.output)
            printer.plain_print(" ")

        printer.emit_Region(stmt.body)

        with printer.rich(style=printer.color.comment):
            printer.plain_print(f" // func.func {stmt.sym_name}")
