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


@dataclass
class PrintState:
    ssa_id: SSAValueSymbolTable = field(default_factory=SSAValueSymbolTable)
    block_id: IdTable[ir.Block] = field(default_factory=IdTable[ir.Block])
    indent: int = 0
    result_width: int = 0
    indent_marks: list[int] = field(default_factory=list)
    result_width: int = 0
    "SSA-value column width in printing"


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
        return

    def emit_Region(self, region: ir.Region) -> None:
        return

    def emit_Block(self, block: ir.Block) -> None:
        return

    def emit_Statement_fallback(self, stmt: ir.Statement) -> None:
        return

    def emit_Attribute_fallback(self, attr: ir.Attribute) -> None:
        if isinstance(attr, ir.TypeAttribute):
            self.print_name(attr)
        else:
            self.print_name(attr)
            self.plain_print("(")
            for idx, f in enumerate(fields(attr)):
                if idx > 0:
                    self.plain_print(", ")
                self.emit(getattr(attr, f.name))
            self.plain_print(")")

    def print_name(self, node: ir.Attribute | ir.Statement, prefix: str = "") -> None:
        self.print_dialect_path(node, prefix=prefix)
        self.console.print(".", node.name, end="", sep="")

    def print_dialect_path(
        self, node: ir.Attribute | ir.Statement, prefix: str = ""
    ) -> None:
        if node.dialect:  # not None
            self.plain_print(prefix)
            self.plain_print(node.dialect.name, style=self.color.dialect)
        else:
            self.plain_print(prefix)

    def plain_print(self, *objects, sep="", end="", style=None, highlight=None):
        self.console.out(*objects, sep=sep, end=end, style=style, highlight=highlight)

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
