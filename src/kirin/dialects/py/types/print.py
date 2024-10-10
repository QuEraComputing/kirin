from kirin.codegen import DialectEmit, Printer, impl

from .base import (
    PyAnyType,
    PyBottomType,
    PyClass,
    PyConst,
    PyGeneric,
    PyLiteral,
    PyTypeVar,
    PyUnion,
    PyVararg,
)
from .dialect import dialect


@dialect.register(key="print")
class Print(DialectEmit):

    @impl(PyClass)
    def print_class(self, printer: Printer, stmt: PyClass):
        printer.plain_print("!")
        printer.plain_print("py", style=printer.color.dialect)
        printer.plain_print(".", stmt.name, ".", stmt.typ.__name__)

    @impl(PyAnyType)
    def print_anytype(self, printer: Printer, stmt: PyAnyType):
        printer.plain_print("!")
        printer.plain_print("py", style=printer.color.dialect)
        printer.plain_print(".Any")

    @impl(PyBottomType)
    def print_bottomtype(self, printer: Printer, stmt: PyBottomType):
        printer.plain_print("!")
        printer.plain_print("py", style=printer.color.dialect)
        printer.plain_print(".Bottom")

    @impl(PyGeneric)
    def print_generic(self, printer: Printer, stmt: PyGeneric):
        printer.emit_Attribute(stmt.body)
        printer.plain_print("[")
        if stmt.vars:
            printer.print_seq(stmt.vars)
        if stmt.vararg is not None:
            if stmt.vars:
                printer.plain_print(", ")
            printer.emit_Attribute(stmt.vararg.typ)
            printer.plain_print(", ...")
        printer.plain_print("]")

    @impl(PyLiteral)
    def print_literal(self, printer: Printer, stmt: PyLiteral):
        printer.plain_print(repr(stmt.data))

    @impl(PyVararg)
    def print_vararg(self, printer: Printer, stmt: PyVararg):
        printer.plain_print("*")
        printer.emit_Attribute(stmt.typ)

    @impl(PyTypeVar)
    def print_typevar(self, printer: Printer, stmt: PyTypeVar):
        printer.plain_print(f"~{stmt.varname}")
        if not stmt.bound.is_top():
            printer.plain_print(" : ")
            printer.emit_Attribute(stmt.bound)

    @impl(PyConst)
    def print_const(self, printer: Printer, stmt: PyConst):
        printer.print_name(stmt, prefix="!")
        printer.plain_print("(", stmt.data, ", ")
        printer.emit_Attribute(stmt.typ)
        printer.plain_print(")")

    @impl(PyUnion)
    def print_union(self, printer: Printer, stmt: PyUnion):
        printer.print_name(stmt, prefix="!")
        printer.print_seq(stmt.args, delim=", ", prefix="[", suffix="]")
