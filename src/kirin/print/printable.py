from __future__ import annotations

from abc import abstractmethod
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from kirin import ir
    from kirin.print import Printer


class Printable:

    @staticmethod
    def __get_printer(
        printer: Printer | None = None, analysis: dict[ir.SSAValue, Any] | None = None
    ) -> Printer:
        if printer is None:
            from kirin.print import Printer

            return Printer(analysis=analysis)
        return printer

    def pager(
        self,
        printer: Printer | None = None,
        analysis: dict[ir.SSAValue, Any] | None = None,
    ) -> None:
        printer = self.__get_printer(printer, analysis)
        with printer.console.pager(styles=True, links=True):
            self.print(printer)

    def print(
        self,
        printer: Printer | None = None,
        analysis: dict[ir.SSAValue, Any] | None = None,
        end: str = "\n",
    ) -> None:
        """
        Entry point of the printing process.

        Args:
            printer (Printer):
                `Printer` object to use for printing.
                If None, a new `Printer` object will be created.
            analysis (dict[ir.SSAValue, Printable]):
                Analysis results to use for printing. If `None`, no analysis results
        """
        printer = self.__get_printer(printer, analysis)
        self.print_impl(printer)
        printer.plain_print(end)

    def print_str(
        self,
        printer: Printer | None = None,
        analysis: dict[ir.SSAValue, Any] | None = None,
        end: str = "\n",
    ) -> str:
        printer = self.__get_printer(printer, analysis)
        with printer.string_io() as stream:
            self.print(printer, analysis=analysis, end=end)
            return stream.getvalue()

    @abstractmethod
    def print_impl(self, printer: Printer) -> None:
        raise NotImplementedError(f"print is not implemented for {type(self)}")
