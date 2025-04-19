from __future__ import annotations

import sys
from typing import TYPE_CHECKING
from dataclasses import dataclass

from kirin.exception import CustomStackTrace

if TYPE_CHECKING:
    from .frame import FrameABC
    from .state import InterpreterState


# errors
class InterpreterError(Exception):
    """Generic interpreter error.

    This is the base class for all interpreter errors. Interpreter
    errors will be catched by the interpreter and handled appropriately
    as an error with stack trace (of Kirin, not Python) from the interpreter.
    """

    pass


@dataclass
class WrapException(InterpreterError):
    """A special interpreter error that wraps a Python exception."""

    exception: Exception


@dataclass
class IntepreterExit(CustomStackTrace):
    exception: Exception
    state: InterpreterState

    def print_stacktrace(self) -> None:
        """Print the stacktrace of the interpreter."""
        frame: FrameABC | None = self.state.current_frame
        print(f"{type(self.exception).__name__}: {self.exception}", file=sys.stderr)
        print("Traceback (most recent call last):", file=sys.stderr)
        frames: list[FrameABC] = []
        while frame is not None:
            frames.append(frame)
            frame = frame.parent
        frames.reverse()
        for frame in frames:
            if stmt := frame.current_stmt:
                print("  " + repr(stmt.source), file=sys.stderr)
                print("     " + stmt.print_str(end=""), file=sys.stderr)


class FuelExhaustedError(InterpreterError):
    """An error raised when the interpreter runs out of fuel."""

    pass
