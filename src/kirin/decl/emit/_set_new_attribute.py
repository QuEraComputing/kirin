"""Copied from dataclasses in Python 3.10.13."""

from types import FunctionType


def set_qualname(cls: type, value):
    # Ensure that the functions returned from _create_fn uses the proper
    # __qualname__ (the class they belong to).
    if isinstance(value, FunctionType):
        value.__qualname__ = f"{cls.__qualname__}.{value.__name__}"
    return value


def set_new_attribute(cls: type, name: str, value):
    # Never overwrites an existing attribute.  Returns True if the
    # attribute already exists.
    if name in cls.__dict__:
        return True
    set_qualname(cls, value)
    setattr(cls, name, value)
    return False
