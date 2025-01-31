import io
from typing import (
    IO,
    TYPE_CHECKING,
    Any,
    Union,
    Generic,
    Literal,
    TypeVar,
    Callable,
    Iterable,
    Optional,
    TypedDict,
)
from contextlib import contextmanager
from dataclasses import field, dataclass

from rich.theme import Theme
from rich.console import Console
from typing_extensions import Unpack

from kirin.idtable import IdTable
from kirin.print.printable import Printable

if TYPE_CHECKING:
    from rich.style import Style

    from kirin import ir

    class RichConsoleOptions(TypedDict, total=False):
        color_system: Optional[
            Literal["auto", "standard", "256", "truecolor", "windows"]
        ]
        force_terminal: Optional[bool]
        force_interactive: Optional[bool]
        soft_wrap: bool
        stderr: bool
        quiet: bool
        width: Optional[int]
        height: Optional[int]
        style: Optional[Style | str]
        no_color: Optional[bool]
        record: bool
        markup: bool
        emoji: bool
        log_time: bool
        log_path: bool
        safe_box: bool


DEFAULT_THEME = {
    "dark": Theme(
        {
            "dialect": "dark_blue",
            "type": "dark_blue",
            "comment": "bright_black",
            "keyword": "red",
            "symbol": "cyan",
            "warning": "yellow",
        }
    ),
    "light": Theme(
        {
            "dialect": "blue",
            "type": "blue",
            "comment": "bright_black",
            "keyword": "red",
            "symbol": "cyan",
            "warning": "yellow",
        }
    ),
}


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
    """printer for IR nodes and other objects."""

    stream: IOType | None = None
    """Output stream"""
    analysis: dict["ir.SSAValue", Any] | None = None
    """Analysis results"""
    console: Console = field(default_factory=Console)
    """Rich console"""
    state: PrintState = field(default_factory=PrintState)
    """Printing state"""
    show_indent_mark: bool = True
    "Whether to show indent marks, e.g │"

    def __init__(
        self,
        stream: IOType | None = None,
        analysis: dict["ir.SSAValue", Printable] | None = None,
        show_indent_mark: bool = True,
        theme: Theme | dict | Literal["dark", "light"] = "dark",
        # NOTE: turn off jupyter rendering cuz we just want text
        force_jupyter: "Optional[bool]" = False,
        **kwargs: Unpack["RichConsoleOptions"],
    ):
        self.stream = stream
        self.analysis = analysis
        self.state = PrintState()
        self.show_indent_mark = show_indent_mark
        if isinstance(theme, dict):
            theme = Theme(theme)
        elif isinstance(theme, str):
            theme = DEFAULT_THEME[theme]

        self.console = Console(
            file=self.stream,
            highlight=False,
            theme=theme,
            force_jupyter=force_jupyter,
            **kwargs,
        )

    def print(self, object):
        """entry point for printing an object

        Args:
            object: object to print.
        """
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
        """print the name of a node

        Args:
            node(ir.Attribute | ir.Statement): node to print
            prefix(str): prefix to print before the name, default to ""
        """
        self.print_dialect_path(node, prefix=prefix)
        if node.dialect:
            self.plain_print(".")
        self.plain_print(node.name)

    def print_dialect_path(
        self, node: Union["ir.Attribute", "ir.Statement"], prefix: str = ""
    ) -> None:
        """print the dialect path of a node.

        Args:
            node(ir.Attribute | ir.Statement): node to print
            prefix(str): prefix to print before the dialect path, default to ""
        """
        if node.dialect:  # not None
            self.plain_print(prefix)
            self.plain_print(node.dialect.name, style="dialect")
        else:
            self.plain_print(prefix)

    def print_newline(self):
        """print a newline character.

        This method also prints any messages in the state for debugging.
        """
        self.plain_print("\n")

        if self.state.messages:
            for message in self.state.messages:
                self.plain_print(message)
                self.plain_print("\n")
            self.state.messages.clear()
        self.print_indent()

    def print_indent(self):
        """print the current indentation level optionally with indent marks."""
        indent_str = ""
        if self.show_indent_mark and self.state.indent_marks:
            indent_str = "".join(
                "│" if i in self.state.indent_marks else " "
                for i in range(self.state.indent)
            )
            with self.rich(style="comment"):
                self.plain_print(indent_str)
        else:
            indent_str = " " * self.state.indent
            self.plain_print(indent_str)

    def plain_print(self, *objects, sep="", end="", style=None, highlight=None):
        """print objects without any formatting.

        Args:
            *objects: objects to print

        Keyword Args:
            sep(str): separator between objects, default to ""
            end(str): end character, default to ""
            style(str): style to use, default to None
            highlight(bool): whether to highlight the text, default to None
        """
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
        """print a sequence of objects.

        Args:
            seq(Iterable[ElemType]): sequence of objects to print

        Keyword Args:
            emit(Callable[[ElemType], None]): function to print each element, default to None
            delim(str): delimiter between elements, default to ", "
            prefix(str): prefix to print before the sequence, default to ""
            suffix(str): suffix to print after the sequence, default to ""
            style(str): style to use, default to None
            highlight(bool): whether to highlight the text, default to None
        """
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
        """print a mapping of key-value pairs.

        Args:
            elems(dict[KeyType, ValueType]): mapping to print

        Keyword Args:
            emit(Callable[[ValueType], None]): function to print each value, default to None
            delim(str): delimiter between key-value pairs, default to ", "
        """
        emit = emit or self.print
        for i, (key, value) in enumerate(elems.items()):
            if i > 0:
                self.plain_print(delim)
            self.plain_print(f"{key}=")
            emit(value)

    @contextmanager
    def align(self, width: int):
        """align the result column width, and restore it after the context.

        Args:
            width(int): width of the column

        Yields:
            PrintState: the state with the new column width
        """
        old_width = self.state.result_width
        self.state.result_width = width
        try:
            yield self.state
        finally:
            self.state.result_width = old_width

    @contextmanager
    def indent(self, increase: int = 2, mark: bool | None = None):
        """increase the indentation level, and restore it after the context.

        Args:
            increase(int): amount to increase the indentation level, default to 2
            mark(bool): whether to mark the indentation level, default to None

        Yields:
            PrintState: the state with the new indentation level.
        """
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
        """set the rich style and highlight, and restore them after the context.

        Args:
            style(str): style to use, default to None
            highlight(bool): whether to highlight the text, default to False

        Yields:
            PrintState: the state with the new style and highlight.
        """
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
        """Temporary string IO for capturing output.

        Yields:
            io.StringIO: the string IO object.
        """
        stream = io.StringIO()
        old_file = self.console.file
        self.console.file = stream
        try:
            yield stream
        finally:
            self.console.file = old_file
            stream.close()

    def result_str(self, results: list["ir.ResultValue"]) -> str:
        """return the string representation of a list of result values.

        Args:
            results(list[ir.ResultValue]): list of result values to print
        """
        with self.string_io() as stream:
            self.print_seq(results, delim=", ")
            result_str = stream.getvalue()
        return result_str

    def debug(self, message: str):
        """Print a debug message."""
        self.state.messages.append(f"DEBUG: {message}")
