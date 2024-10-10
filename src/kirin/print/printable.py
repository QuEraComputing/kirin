from __future__ import annotations

from abc import abstractmethod
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from kirin.print import Printer


class Printable:

    def print(self, printer: Printer | None = None) -> None:
        """entry point of the printing process."""
        if printer is None:
            from kirin.print import Printer

            printer = Printer()
        self.print_impl(printer)
        printer.plain_print("\n")  # add a new line in the end

    def print_str(self, printer: Printer) -> str:
        with printer.string_io() as stream:
            self.print(printer)
            return stream.getvalue()

    @abstractmethod
    def print_impl(self, printer: Printer) -> None:
        raise NotImplementedError(f"print is not implemented for {type(self)}")
