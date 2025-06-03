"""This module contains custom exception handling
for the Kirin-based compilers.
"""

from __future__ import annotations

import os
import sys
import math
import types
import shutil
import textwrap
from typing import TYPE_CHECKING

from rich.console import Console

from kirin.source import SourceInfo

if TYPE_CHECKING:
    from kirin import interp

KIRIN_INTERP_STATE = "__kirin_interp_state"
KIRIN_PYTHON_STACKTRACE = os.environ.get("KIRIN_PYTHON_STACKTRACE", "0") == "1"
KIRIN_STATIC_CHECK_LINENO = os.environ.get("KIRIN_STATIC_CHECK_LINENO", "1") == "1"
KIRIN_STATIC_CHECK_INDENT = int(os.environ.get("KIRIN_STATIC_CHECK_INDENT", "2"))
KIRIN_STATIC_CHECK_MAX_LINES = int(os.environ.get("KIRIN_STATIC_CHECK_MAX_LINES", "3"))


class StaticCheckError(Exception):
    def __init__(self, *messages: str, help: str | None = None) -> None:
        super().__init__(*messages)
        self.help: str | None = help
        self.source: SourceInfo | None = None
        self.lines: list[str] | None = None
        self.indent: int = KIRIN_STATIC_CHECK_INDENT
        self.max_lines: int = KIRIN_STATIC_CHECK_MAX_LINES
        self.show_lineno: bool = KIRIN_STATIC_CHECK_LINENO

    def hint(self):
        help = self.help or ""
        source = self.source or SourceInfo(0, 0, 0, 0)
        lines = self.lines or []
        begin = max(0, source.lineno - self.max_lines)
        end = max(
            max(source.lineno + self.max_lines, source.end_lineno or 1),
            1,
        )
        end = min(len(lines), end)  # make sure end is within bounds
        lines = lines[begin:end]
        error_lineno = source.lineno + source.lineno_begin
        error_lineno_len = len(str(error_lineno))
        code_indent = min(map(self.__get_indent, lines), default=0)

        console = Console(force_terminal=True)
        with console.capture() as capture:
            console.print(
                f"  {source or 'stdin'}",
                markup=True,
                highlight=False,
            )
            for lineno, line in enumerate(lines, begin):
                line = " " * self.indent + line[code_indent:]
                if self.show_lineno:
                    if lineno + 1 == source.lineno:
                        line = f"{error_lineno}[dim]│[/dim]" + line
                    else:
                        line = "[dim]" + " " * (error_lineno_len) + "│[/dim]" + line
                console.print("  " + line, markup=True, highlight=False)
                if lineno + 1 == source.lineno:
                    console.print(
                        "  "
                        + self.__arrow(
                            source,
                            code_indent,
                            error_lineno_len,
                            help,
                            self.indent,
                            self.show_lineno,
                        ),
                        markup=True,
                        highlight=False,
                    )
        return capture.get()

    def __arrow(
        self,
        source: SourceInfo,
        code_indent: int,
        error_lineno_len: int,
        help,
        indent: int,
        show_lineno: bool,
    ) -> str:
        ret = " " * (source.col_offset - code_indent)
        if source.end_col_offset:
            ret += "^" * (source.end_col_offset - source.col_offset)
        else:
            ret += "^"

        ret = " " * indent + "[red]" + ret
        if help:
            hint_indent = len(ret) - len("[ret]") + len(" help: ")
            terminal_width = math.floor(shutil.get_terminal_size().columns * 0.7)
            terminal_width = max(terminal_width - hint_indent, 10)
            wrapped = textwrap.fill(str(help), width=terminal_width)
            lines = wrapped.splitlines()
            ret += " help: " + lines[0] + "[/red]"
            for line in lines[1:]:
                ret += (
                    "\n"
                    + " " * (error_lineno_len + indent)
                    + "[dim]│[/dim]"
                    + " " * hint_indent
                    + "[red]"
                    + line
                    + "[/red]"
                )
        if show_lineno:
            ret = " " * error_lineno_len + "[dim]│[/dim]" + ret
        return ret

    @staticmethod
    def __get_indent(line: str) -> int:
        if len(line) == 0:
            return int(1e9)  # very large number
        return len(line) - len(line.lstrip())


def enable_stracetrace():
    """Enable the stacktrace for all exceptions."""
    global KIRIN_PYTHON_STACKTRACE
    KIRIN_PYTHON_STACKTRACE = True


def disable_stracetrace():
    """Disable the stacktrace for all exceptions."""
    global KIRIN_PYTHON_STACKTRACE
    KIRIN_PYTHON_STACKTRACE = False


def print_stacktrace(exception: Exception, state: interp.InterpreterState):
    frame: interp.FrameABC | None = state._current_frame
    print(
        "==== Python stacktrace has been disabled for simplicity, set KIRIN_PYTHON_STACKTRACE=1 to enable it ===="
    )
    print(f"{type(exception).__name__}: {exception}", file=sys.stderr)
    print("Traceback (most recent call last):", file=sys.stderr)
    frames: list[interp.FrameABC] = []
    while frame is not None:
        frames.append(frame)
        frame = frame.parent
    frames.reverse()
    for frame in frames:
        if stmt := frame.current_stmt:
            print("  " + repr(stmt.source), file=sys.stderr)
            print("     " + stmt.print_str(end=""), file=sys.stderr)


def exception_handler(exc_type, exc_value, exc_tb: types.TracebackType):
    """Custom exception handler to format and print exceptions."""
    if not KIRIN_PYTHON_STACKTRACE and issubclass(exc_type, StaticCheckError):
        console = Console(force_terminal=True)
        with console.capture() as capture:
            console.print(
                "==== Python stacktrace has been disabled for simplicity, set KIRIN_PYTHON_STACKTRACE=1 to enable it ===="
            )
            console.print(f"[bold red]{exc_type.__name__}:[/bold red]", end="")
        print(capture.get(), *exc_value.args, file=sys.stderr)
        print("Source Traceback:", file=sys.stderr)
        print(exc_value.hint(), file=sys.stderr, end="")
        return

    if (
        not KIRIN_PYTHON_STACKTRACE
        and (state := getattr(exc_value, KIRIN_INTERP_STATE, None)) is not None
    ):
        # Handle custom stack trace exceptions
        print_stacktrace(exc_value, state)
        return

    # Call the default exception handler
    sys.__excepthook__(exc_type, exc_value, exc_tb)


# Set the custom exception handler
sys.excepthook = exception_handler


def custom_exc(shell, etype, evalue, tb, tb_offset=None):
    if issubclass(etype, StaticCheckError):
        # Handle BuildError exceptions
        print(evalue, file=sys.stderr)
        return
    shell.showtraceback((etype, evalue, tb), tb_offset=tb_offset)


try:
    ip = get_ipython()  # type: ignore
    # Register your custom exception handler
    ip.set_custom_exc((Exception,), custom_exc)
except NameError:
    # Not in IPython, so we won't set the custom exception handler
    pass
