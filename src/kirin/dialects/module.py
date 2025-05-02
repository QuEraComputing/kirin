"""Module dialect provides a simple module
that is roughly a list of function statements.

This dialect provides the dialect necessary for compiling a function into
lower-level IR with all its callee functions.
"""

from __future__ import annotations

from dataclasses import dataclass

from kirin import ir, types, interp
from kirin.decl import info, statement
from kirin.print import Printer
from kirin.analysis import TypeInference

dialect = ir.Dialect("module")


@dataclass(frozen=True)
class ModuleEntryPoint(ir.EntryPointInterface["Module"]):
    def get_entry_point_symbol(self, stmt: Module) -> str:
        return stmt.entry

    def get_entry_point(self, stmt: Module) -> ir.Statement:
        name = self.get_entry_point_symbol(stmt)
        for node in stmt.body.blocks[0].stmts:
            trait = node.get_trait(ir.SymbolOpInterface)
            if trait is None:
                continue

            node_name = trait.get_sym_name(node).unwrap()
            if node_name == name:
                return node
        raise ir.ValidationError(stmt, "entry point not found")


@statement(dialect=dialect)
class Module(ir.Statement):
    traits = frozenset(
        {
            ir.IsolatedFromAbove(),
            ir.SymbolTable(),
            ir.SymbolOpInterface(),
            ModuleEntryPoint(),
        }
    )
    sym_name: str = info.attribute()
    entry: str = info.attribute()
    body: ir.Region = info.region(multi=False)

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self)
        printer.plain_print(" ")
        with printer.rich(style="symbol"):
            printer.plain_print("@", self.sym_name)
        printer.plain_print(" {")
        with printer.indent():
            for idx, stmt in enumerate(self.body.blocks[0].stmts):
                printer.print_newline()
                stmt.print(printer)
        printer.print_newline()
        printer.plain_print("}")
        with printer.rich(style="comment"):
            printer.plain_print(" // entry: ", self.entry)


@statement(dialect=dialect)
class Invoke(ir.Statement):
    """A special statement that represents
    a function calling functions by symbol name.

    Note:
        This statement is here for completeness, for interpretation,
        it is recommended to rewrite this statement into a `func.Invoke`
        after looking up the symbol table.
    """

    traits = frozenset({ir.MaybePure()})
    callee: str = info.attribute()
    inputs: tuple[ir.SSAValue, ...] = info.argument()
    kwargs: tuple[ir.SSAValue, ...] = info.argument()
    keys: tuple[str, ...] = info.attribute(default=())
    result: ir.ResultValue = info.result()
    purity: bool = info.attribute(default=False)

    def print_impl(self, printer: Printer) -> None:
        with printer.rich(style="red"):
            printer.print_name(self)
        printer.plain_print(" ")

        with printer.rich(style="symbol"):
            printer.plain_print("@", self.callee)
        kwargs = dict(zip(self.keys, self.kwargs))
        printer.plain_print("(")
        printer.print_seq(self.inputs)
        if kwargs and self.inputs:
            printer.plain_print(", ")
        printer.print_mapping(kwargs, delim=", ")
        printer.plain_print(")")

        with printer.rich(style="comment"):
            printer.plain_print(" : ")
            printer.print_seq(
                [result.type for result in self._results],
                delim=", ",
            )
            printer.plain_print(f" maybe_pure={self.purity}")

    def check(self) -> None:
        assert len(self.keys) == len(
            self.kwargs
        ), "keys and kwargs must have the same length"


@dialect.register
class Concrete(interp.MethodTable):

    @interp.impl(Invoke)
    def interp_Invoke(
        self, interp_: interp.Interpreter, frame: interp.Frame, stmt: Invoke
    ):
        callee = interp_.symbol_table.get(stmt.callee)
        if callee is None:
            raise interp.InterpreterError(f"symbol {stmt.callee} not found")

        _, ret = interp_.call(
            callee, ir.Method(interp_.dialects, callee), *frame.get_values(stmt.inputs)
        )
        return (ret,)


@dialect.register(key="typeinfer")
class TypeInfer(interp.MethodTable):

    @interp.impl(Invoke)
    def typeinfer_Invoke(
        self, interp_: TypeInference, frame: interp.Frame, stmt: Invoke
    ):
        callee = interp_.symbol_table.get(stmt.callee)
        if callee is None:
            return (types.Bottom,)

        callee = interp_.symbol_table.get(stmt.callee)
        if callee is None:
            raise interp.InterpreterError(f"symbol {stmt.callee} not found")

        _, ret = interp_.call(
            callee,
            interp_.method_self(ir.Method(interp_.dialects, callee)),
            *frame.get_values(stmt.inputs),
        )
        return (ret,)
