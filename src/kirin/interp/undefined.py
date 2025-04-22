"""This module provides a singleton class `Undefined` and `UndefinedType`
that represents an undefined value in the Kirin interpreter.

The `Undefined` class is a singleton that can be used to represent an
undefined value in the interpreter. It is used to indicate that a value
has not been set or is not available. This is used to distinguish between
an undefined value and a Python `None` value.
"""

from typing_extensions import TypeIs


class UndefinedMeta(type):

    def __init__(cls, name, bases, attrs):
        super().__init__(name, bases, attrs)
        cls._instance = None

    def __call__(cls):
        if cls._instance is None:
            cls._instance = super().__call__()
        return cls._instance


class UndefinedType(metaclass=UndefinedMeta):
    pass


Undefined = UndefinedType()
"""Singleton instance of `UndefinedType` that represents an undefined value."""


def is_undefined(value: object) -> TypeIs[UndefinedType]:
    """Check if the given value is an instance of `UndefinedType`.

    Args:
        value (object): The value to check.

    Returns:
        bool: True if the value is an instance of `UndefinedType`, False otherwise.
    """
    return value is Undefined
