from __future__ import annotations


# errors
class InterpreterError(Exception):
    """Generic interpreter error.

    This is the base class for all interpreter errors.
    """

    pass


class StackOverflowError(InterpreterError):
    """An error raised when the interpreter stack overflows."""

    pass


class FuelExhaustedError(InterpreterError):
    """An error raised when the interpreter runs out of fuel."""

    pass
