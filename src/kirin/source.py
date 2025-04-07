import ast
from dataclasses import dataclass

from rich.console import Console


@dataclass
class SourceInfo:
    lineno: int
    col_offset: int
    end_lineno: int | None
    end_col_offset: int | None

    @classmethod
    def from_ast(cls, node: ast.AST, lineno_offset: int = 0, col_offset: int = 0):
        end_lineno = getattr(node, "end_lineno", None)
        end_col_offset = getattr(node, "end_col_offset", None)
        return cls(
            getattr(node, "lineno", 0) + lineno_offset,
            getattr(node, "col_offset", 0) + col_offset,
            end_lineno + lineno_offset if end_lineno is not None else None,
            end_col_offset + col_offset if end_col_offset is not None else None,
        )

    def offset(self, lineno_offset: int = 0, col_offset: int = 0):
        """Offset the source info by the given offsets.

        Args:
            lineno_offset (int): The line number offset.
            col_offset (int): The column offset.
        """
        self.lineno += lineno_offset
        self.col_offset += col_offset
        if self.end_lineno is not None:
            self.end_lineno += lineno_offset
        if self.end_col_offset is not None:
            self.end_col_offset += col_offset
        return self

    def error_hint(
        self,
        lines: list[str],
        err: Exception,
        *,
        file: str | None = None,
        indent: int = 2,
        show_lineno: bool = True,
        max_lines: int = 3,
        lineno_offset: int = 0,
    ) -> str:
        """Generate a hint for the error.

        Args:
            lines (list[str]): The lines of code.
            err (Exception): The error to display. If the error object has a
                `help` attribute, it will be used as the help message at the
                location of the error.
            file (str | None): The name of the file.
            indent (int): The indentation level.
            show_lineno (bool): Whether to show the line number.
            max_lines (int): The maximum number of lines to display.
            lineno_offset (int): The offset for the line number.

        Returns:
            str: The hint for the error.
        """
        help = getattr(err, "help", None)
        begin = max(0, self.lineno - max_lines - lineno_offset)
        end = max(
            max(self.lineno + max_lines, self.end_lineno or 0) - lineno_offset,
            0,
        )
        end = min(len(lines), end)  # make sure end is within bounds
        lines = lines[begin:end]
        error_lineno = self.lineno - lineno_offset - 1
        error_lineno_len = len(str(self.lineno))
        code_indent = min(map(self.__get_indent, lines), default=0)

        console = Console(force_terminal=True)
        with console.capture() as capture:
            console.print()
            console.print(
                f"File: [dim]{file or 'stdin'}:{self.lineno}[/dim]",
                markup=True,
                highlight=False,
            )
            emsg = "\n  ".join(err.args)
            console.print(f"[red]  {type(err).__name__}: {emsg}[/red]")
            for lineno, line in enumerate(lines, begin):
                line = " " * indent + line[code_indent:]
                if show_lineno:
                    if lineno == error_lineno:
                        line = f"{self.lineno}[dim]│[/dim]" + line
                    else:
                        line = "[dim]" + " " * (error_lineno_len) + "│[/dim]" + line
                console.print("  " + line, markup=True, highlight=False)
                if lineno == error_lineno:
                    console.print(
                        "  "
                        + self.__arrow(
                            code_indent, error_lineno_len, help, indent, show_lineno
                        ),
                        markup=True,
                        highlight=False,
                    )

            if end == error_lineno:
                console.print(
                    "  "
                    + self.__arrow(
                        code_indent, error_lineno_len, help, indent, show_lineno
                    ),
                    markup=True,
                    highlight=False,
                )

        return capture.get()

    def __arrow(
        self,
        code_indent: int,
        error_lineno_len: int,
        help,
        indent: int,
        show_lineno: bool,
    ) -> str:
        ret = " " * (self.col_offset - code_indent)
        if self.end_col_offset:
            ret += "^" * (self.end_col_offset - self.col_offset)
        else:
            ret += "^"

        ret = " " * indent + "[red]" + ret
        if help:
            ret += " help: " + str(help) + "[/red]"
        if show_lineno:
            ret = " " * error_lineno_len + "[dim]│[/dim]" + ret
        return ret

    @staticmethod
    def __get_indent(line: str) -> int:
        if len(line) == 0:
            return int(1e9)  # very large number
        return len(line) - len(line.lstrip())
