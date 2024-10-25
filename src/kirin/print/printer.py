import io
from contextlib import contextmanager
from dataclasses import dataclass, field
from typing import IO, TYPE_CHECKING, Any, Callable, Generic, Iterable, TypeVar, Union

from rich.console import Console

from kirin.idtable import IdTable
from kirin.print.printable import Printable

if TYPE_CHECKING:
    from kirin import ir


@dataclass
class ColorScheme:
    dialect: str = "dark_blue"
    type: str = "dark_blue"
    comment: str = "bright_black"
    keyword: str = "red"
    symbol: str = "cyan"
    warning: str = "yellow"


@dataclass
class PrintState:
    ssa_id: IdTable["ir.SSAValue"] = field(default_factory=IdTable["ir.SSAValue"])
    block_id: IdTable["ir.Block"] = field(
        default_factory=lambda: IdTable["ir.Block"](prefix="^")
    )
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
class Printer(Generic[IOType]):
    stream: IOType | None = None
    analysis: dict["ir.SSAValue", Any] | None = None
    console: Console = field(default_factory=Console)
    state: PrintState = field(default_factory=PrintState)
    color: ColorScheme = field(default_factory=ColorScheme)
    show_indent_mark: bool = True
    "Whether to show indent marks, e.g │"

    def __init__(
        self,
        stream: IOType | None = None,
        analysis: dict["ir.SSAValue", Printable] | None = None,
        show_indent_mark: bool = True,
    ):
        self.stream = stream
        self.analysis = analysis
        self.console = Console(file=self.stream, highlight=False)
        self.state = PrintState()
        self.color = ColorScheme()
        self.show_indent_mark = show_indent_mark

    def print(self, object):
        """entry point for printing an object"""
        if isinstance(object, Printable):
            object.print_impl(self)
        else:
            fn = getattr(self, f"print_{object.__class__.__name__}", None)
            if fn is None:
                raise NotImplementedError(
                    f"Printer for {object.__class__.__name__} not found"
                )
            fn(object)

    def print_name(
        self, node: Union["ir.Attribute", "ir.Statement"], prefix: str = ""
    ) -> None:
        self.print_dialect_path(node, prefix=prefix)
        if node.dialect:
            self.plain_print(".")
        self.plain_print(node.name)

    def print_dialect_path(
        self, node: Union["ir.Attribute", "ir.Statement"], prefix: str = ""
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
        emit = emit or self.print
        self.plain_print(prefix, style=style, highlight=highlight)
        for idx, item in enumerate(seq):
            if idx > 0:
                self.plain_print(delim)
            emit(item)
        self.plain_print(suffix, style=style, highlight=highlight)

    KeyType = TypeVar("KeyType")
    ValueType = TypeVar("ValueType", bound=Printable)

    def print_mapping(
        self,
        elems: dict[KeyType, ValueType],
        *,
        emit: Callable[[ValueType], None] | None = None,
        delim: str = ", ",
    ):
        emit = emit or self.print
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

    def result_str(self, results: list["ir.ResultValue"]) -> str:
        with self.string_io() as stream:
            self.print_seq(results, delim=", ")
            result_str = stream.getvalue()
        return result_str

    def debug(self, message: str):
        self.state.messages.append(f"DEBUG: {message}")
