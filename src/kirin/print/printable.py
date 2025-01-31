from __future__ import annotations

from abc import abstractmethod
from typing import TYPE_CHECKING

# NOTE: we don't want to actually load rich here
if TYPE_CHECKING:
    from typing import IO, Literal, Optional, TypedDict

    from rich.theme import Theme
    from typing_extensions import Unpack

    from kirin import ir
    from kirin.print import Printer

    from .printer import RichConsoleOptions

    class _PrintOptions(TypedDict, total=False):
        stream: Optional[IO[str]]
        analysis: Optional[dict["ir.SSAValue", Printable]]
        show_indent_mark: bool
        theme: Theme | dict | Literal["dark", "light"]
        force_jupyter: Optional[bool]

    class PrintOptions(_PrintOptions, RichConsoleOptions):
        pass


KEYWORD_DOC = """
Keyword Args:
    stream (IO[str]):
        The stream to write the output to. If None, the output will
        be written to stdout.
    analysis (dict[ir.SSAValue, Printable]):
        Analysis results to use for printing. If `None`, no analysis results
    show_indent_mark (bool):
        Whether to show the indentation mark.
    theme (Theme | dict | str):
        The theme to use for printing, defaults to "dark".
"""


class Printable:
    """Base class for all objects that can be pretty printed.

    This class provides an interface for pretty printing objects. The
    `print` method is the entry point for the printing process. The
    `print_impl` method is the implementation of the printing process
    and should be implemented by the derived classes.
    """

    @staticmethod
    def __get_printer(
        printer: Printer | None = None,
        **options: Unpack["PrintOptions"],
    ) -> Printer:
        if printer is None:
            from kirin.print import Printer

            return Printer(**options)
        return printer

    def pager(
        self,
        printer: Printer | None = None,
        **options: Unpack["PrintOptions"],
    ) -> None:
        """Pretty print the object with a pager.

        Args:
            printer (Printer):
                `Printer` object to use for printing.
                If None, a new `Printer` object will be created.

        Keyword Args:
            stream (IO[str]):
                The stream to write the output to. If None, the output will
                be written to stdout.
            analysis (dict[ir.SSAValue, Printable]):
                Analysis results to use for printing. If `None`, no analysis results
            show_indent_mark (bool):
                Whether to show the indentation mark.
            theme (Theme | dict | str):
                The theme to use for printing, defaults to "dark".

        !!! note
            This function also accepts all other `rich.console.Console` options.
        """
        printer = self.__get_printer(printer, **options)
        with printer.console.pager(styles=True, links=True):
            self.print(printer)

    def print(
        self,
        printer: Printer | None = None,
        end: str = "\n",
        **options: Unpack["PrintOptions"],
    ) -> None:
        """
        Entry point of the printing process.

        Args:
            printer (Printer):
                `Printer` object to use for printing.
                If None, a new `Printer` object will be created.

        Keyword Args:
            stream (IO[str]):
                The stream to write the output to. If None, the output will
                be written to stdout.
            analysis (dict[ir.SSAValue, Printable]):
                Analysis results to use for printing. If `None`, no analysis results
            show_indent_mark (bool):
                Whether to show the indentation mark.
            theme (Theme | dict | str):
                The theme to use for printing, defaults to "dark".

        !!! note
            This function also accepts all other `rich.console.Console` options.
        """
        printer = self.__get_printer(printer, **options)
        self.print_impl(printer)
        printer.plain_print(end)

    def print_str(
        self,
        printer: Printer | None = None,
        end: str = "\n",
        **options: Unpack["PrintOptions"],
    ) -> str:
        """Print the object to a string.

        Args:
            printer (Printer):
                `Printer` object to use for printing.
                If None, a new `Printer` object will be created.

        Keyword Args:
            stream (IO[str]):
                The stream to write the output to. If None, the output will
                be written to stdout.
            analysis (dict[ir.SSAValue, Printable]):
                Analysis results to use for printing. If `None`, no analysis results
            show_indent_mark (bool):
                Whether to show the indentation mark.
            theme (Theme | dict | str):
                The theme to use for printing, defaults to "dark".

        !!! note
            This function also accepts all other `rich.console.Console` options.
        """
        printer = self.__get_printer(printer, **options)
        with printer.string_io() as stream:
            self.print(printer, end=end, **options)
            return stream.getvalue()

    @abstractmethod
    def print_impl(self, printer: Printer) -> None:
        raise NotImplementedError(f"print is not implemented for {type(self)}")
