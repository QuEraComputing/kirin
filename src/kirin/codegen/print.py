import io
from contextlib import contextmanager
from dataclasses import dataclass, field, fields
from typing import IO, Callable, Generic, Iterable, TypeVar

from rich.console import Console

from kirin import ir

from .base import CodeGen
from .ssa import IdTable, SSAValueSymbolTable


@dataclass
class ColorScheme:
    dialect: str = "dark_blue"
    type: str = "dark_blue"
    comment: str = "bright_black"
    keyword: str = "red"
    symbol: str = "cyan"


@dataclass
class PrintState:
    ssa_id: SSAValueSymbolTable = field(default_factory=SSAValueSymbolTable)
    block_id: IdTable[ir.Block] = field(default_factory=IdTable[ir.Block])
    indent: int = 0
    result_width: int = 0
    indent_marks: list[int] = field(default_factory=list)
    result_width: int = 0
    "SSA-value column width in printing"
    rich_style: str | None = None
    rich_highlight: bool | None = False
    messages: list[str] = field(default_factory=list)


IOType = TypeVar("IOType", bound=IO)


@dataclass(init=False)
class Printer(CodeGen[None], Generic[IOType]):
    keys = ["print"]
    stream: IOType | None = None
    console: Console = field(default_factory=Console)
    state: PrintState = field(default_factory=PrintState)
    color: ColorScheme = field(default_factory=ColorScheme)
    show_indent_mark: bool = True
    "Whether to show indent marks, e.g │"

    def __init__(
        self,
        dialects: ir.DialectGroup | Iterable[ir.Dialect],
        stream: IOType | None = None,
        show_indent_mark: bool = True,
    ):
        super().__init__(dialects)
        self.stream = stream
        self.console = Console(file=self.stream, highlight=False)
        self.state = PrintState()
        self.color = ColorScheme()
        self.show_indent_mark = show_indent_mark

    def emit_Method(self, mt: ir.Method) -> None:
        return self.emit_Statement(mt.code)

    def emit_Region(self, region: ir.Region) -> None:
        self.plain_print("{")
        if len(region.blocks) == 0:
            self.print_newline()
            self.plain_print("}")
            return

        result_width = 0
        for bb in region.blocks:
            for stmt in bb.stmts:
                result_width = max(result_width, len(self.result_str(stmt._results)))

        with self.align(result_width):
            with self.indent(increase=2, mark=True):
                self.print_newline()
                for bb in region.blocks:
                    self.emit_Block(bb)

        self.print_newline()
        self.plain_print("}")

    def emit_Block(self, block: ir.Block) -> None:
        self.plain_print(f"^{self.state.block_id[block]}")
        self.print_seq(
            [self.state.ssa_id.get_name(arg) for arg in block.args],
            delim=", ",
            prefix="(",
            suffix="):",
            emit=self.plain_print,
        )

        # self.debug(f"indent: {self.state.indent}")
        with self.indent(increase=2, mark=False):
            for stmt in block.stmts:
                self.print_newline()
                if stmt._results:
                    result_str = self.result_str(stmt._results)
                    self.plain_print(result_str.rjust(self.state.result_width), " = ")
                elif self.state.result_width:
                    self.plain_print(" " * self.state.result_width, "   ")
                with self.indent(self.state.result_width + 3, mark=True):
                    self.emit_Statement(stmt)

        self.print_newline()

    def emit_Statement_fallback(self, stmt: ir.Statement) -> None:
        from kirin.decl import fields as stmt_fields

        # self.debug(f"indent: {self.state.indent}")
        # self.debug(f"indent_marks: {self.state.indent_marks}")
        # self.debug(f"indent: {self.state.indent}")
        self.print_name(stmt)
        self.plain_print("(")
        for idx, (name, s) in enumerate(stmt._name_args_slice.items()):
            values = stmt.args[s]
            if (fields := stmt_fields(stmt)) and not fields.args[name].print:
                pass
            else:
                with self.rich(style="orange4"):
                    self.plain_print(name, "=")

            if isinstance(values, ir.SSAValue):
                self.print_SSAValue(values)
            else:
                self.print_seq(values, emit=self.print_SSAValue, delim=", ")

            if idx < len(stmt._name_args_slice) - 1:
                self.plain_print(", ")

        # NOTE: args are specified manually without names
        if not stmt._name_args_slice and stmt._args:
            self.print_seq(stmt._args, emit=self.print_SSAValue, delim=", ")

        self.plain_print(")")

        if stmt.successors:
            self.print_successors(stmt.successors)

        if stmt.properties:
            self.plain_print("<{")
            with self.rich(highlight=True):
                self.print_mapping(
                    stmt.properties, emit=self.emit_Attribute, delim=", "
                )
            self.plain_print("}>")

        if stmt.regions:
            self.print_seq(
                stmt.regions, emit=self.emit_Region, delim=" ", prefix=" (", suffix=")"
            )

        if stmt.attributes:
            self.plain_print("{")
            with self.rich(highlight=True):
                self.print_mapping(
                    stmt.attributes, emit=self.emit_Attribute, delim=", "
                )
            self.plain_print("}")

        if stmt._results:
            with self.rich(style="black"):
                self.plain_print(" -> ")
                self.print_seq(
                    [result.type for result in stmt._results],
                    emit=self.emit_Attribute,
                    delim=", ",
                )

    def emit_Attribute_fallback(self, attr: ir.Attribute) -> None:
        if isinstance(attr, ir.TypeAttribute):
            self.print_name(attr)
        else:
            self.print_name(attr)
            self.plain_print("(")
            values = {}
            for f in fields(attr):
                value = getattr(attr, f.name)
                if isinstance(value, ir.Attribute):
                    values[f.name] = value
                elif isinstance(value, tuple) and all(
                    isinstance(v, ir.Attribute) for v in value
                ):
                    values[f.name] = value

            def _emit_attr(attr: tuple[ir.Attribute, ...] | ir.Attribute):
                if isinstance(attr, tuple):
                    self.plain_print("(")
                    self.print_seq(attr, emit=self.emit_Attribute, delim=", ")
                    self.plain_print(")")
                else:
                    self.emit_Attribute(attr)

            self.print_mapping(values, emit=_emit_attr, delim=", ")
            self.plain_print(")")

    def print_name(self, node: ir.Attribute | ir.Statement, prefix: str = "") -> None:
        self.print_dialect_path(node, prefix=prefix)
        if node.dialect:
            self.plain_print(".")
        self.plain_print(node.name)

    def print_successors(
        self, successors: Iterable[ir.Block], prefix: str = "[", suffix: str = "]"
    ) -> None:
        successors_names = [
            f"^{self.state.block_id[successor]}" for successor in successors
        ]
        self.print_seq(
            successors_names,
            emit=self.plain_print,
            delim=", ",
            prefix=prefix,
            suffix=suffix,
        )

    def print_dialect_path(
        self, node: ir.Attribute | ir.Statement, prefix: str = ""
    ) -> None:
        if node.dialect:  # not None
            self.plain_print(prefix)
            self.plain_print(node.dialect.name, style=self.color.dialect)
        else:
            self.plain_print(prefix)

    def print_newline(self):
        self.plain_print("\n")

        if self.state.messages:
            for message in self.state.messages:
                self.plain_print(message)
                self.plain_print("\n")
            self.state.messages.clear()
        self.print_indent()

    def print_indent(self):
        indent_str = ""
        if self.show_indent_mark and self.state.indent_marks:
            indent_str = "".join(
                "│" if i in self.state.indent_marks else " "
                for i in range(self.state.indent)
            )
            with self.rich(style=self.color.comment):
                self.plain_print(indent_str)
        else:
            indent_str = " " * self.state.indent
            self.plain_print(indent_str)

    def plain_print(self, *objects, sep="", end="", style=None, highlight=None):
        self.console.out(
            *objects,
            sep=sep,
            end=end,
            style=style or self.state.rich_style,
            highlight=highlight or self.state.rich_highlight,
        )

    ElemType = TypeVar("ElemType")

    def print_seq(
        self,
        seq: Iterable[ElemType],
        *,
        emit: Callable[[ElemType], None] | None = None,
        delim: str = ", ",
        prefix: str = "",
        suffix: str = "",
        style=None,
        highlight=None,
    ) -> None:
        emit = emit or self.emit
        self.plain_print(prefix, style=style, highlight=highlight)
        for idx, item in enumerate(seq):
            if idx > 0:
                self.plain_print(delim)
            emit(item)
        self.plain_print(suffix, style=style, highlight=highlight)

    KeyType = TypeVar("KeyType")
    ValueType = TypeVar("ValueType")

    def print_mapping(
        self,
        elems: dict[KeyType, ValueType],
        *,
        emit: Callable[[ValueType], None] | None = None,
        delim: str = ", ",
    ):
        emit = emit or self.emit
        for i, (key, value) in enumerate(elems.items()):
            if i > 0:
                self.plain_print(delim)
            self.plain_print(f"{key}=")
            emit(value)

    @contextmanager
    def align(self, width: int):
        old_width = self.state.result_width
        self.state.result_width = width
        try:
            yield self.state
        finally:
            self.state.result_width = old_width

    @contextmanager
    def indent(self, increase: int = 2, mark: bool | None = None):
        mark = mark if mark is not None else self.show_indent_mark
        self.state.indent += increase
        if mark:
            self.state.indent_marks.append(self.state.indent)
        try:
            yield self.state
        finally:
            self.state.indent -= increase
            if mark:
                self.state.indent_marks.pop()

    @contextmanager
    def rich(self, style: str | None = None, highlight: bool = False):
        old_style = self.state.rich_style
        old_highlight = self.state.rich_highlight
        self.state.rich_style = style
        self.state.rich_highlight = highlight
        try:
            yield self.state
        finally:
            self.state.rich_style = old_style
            self.state.rich_highlight = old_highlight

    @contextmanager
    def string_io(self):
        stream = io.StringIO()
        old_file = self.console.file
        self.console.file = stream
        try:
            yield stream
        finally:
            self.console.file = old_file
            stream.close()

    def print_SSAValue(self, result: ir.SSAValue) -> None:
        self.plain_print(self.state.ssa_id.get_name(result))

    def result_str(self, results: list[ir.ResultValue]) -> str:
        with self.string_io() as stream:
            self.print_seq(results, emit=self.print_SSAValue, delim=", ")
            result_str = stream.getvalue()
        return result_str

    def debug(self, message: str):
        self.state.messages.append(f"DEBUG: {message}")
