from dataclasses import dataclass


# errors
class InterpreterError(Exception):
    pass


@dataclass
class WrapException(InterpreterError):
    exception: Exception


class FuelExhaustedError(InterpreterError):
    pass
