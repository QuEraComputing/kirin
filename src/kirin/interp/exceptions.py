from dataclasses import dataclass


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


class FuelExhaustedError(InterpreterError):
    """An error raised when the interpreter runs out of fuel."""

    pass
