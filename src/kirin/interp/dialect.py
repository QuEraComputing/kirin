import inspect
from abc import ABC
from typing import TYPE_CHECKING, TypeVar, ClassVar
from dataclasses import dataclass

from kirin.interp.base import BaseInterpreter
from kirin.interp.impl import ImplDef, AttributeImplDef

if TYPE_CHECKING:
    from kirin.ir import Attribute
    from kirin.interp.base import BaseInterpreter
    from kirin.interp.impl import Signature, MethodFunction, AttributeFunction


InterpreterType = TypeVar("InterpreterType", bound="BaseInterpreter")
ValueType = TypeVar("ValueType")


@dataclass
class MethodTable(ABC):
    """Base class to define lookup tables for interpreting code for IR nodes in a dialect."""

    table: ClassVar[dict["Signature", "MethodFunction"]]
    """Lookup table for interpreting code for IR nodes in a dialect."""
    attribute: ClassVar[dict[type["Attribute"], "AttributeFunction"]]
    """Lookup table for interpreting code for IR attributes in a dialect."""

    def __init_subclass__(cls) -> None:
        # init the subclass first
        super().__init_subclass__()
        cls.table = {}
        for _, value in inspect.getmembers(cls):
            if isinstance(value, ImplDef):
                for sig in value.signature:
                    cls.table[sig] = value.impl

        cls.attribute = {}
        for _, value in inspect.getmembers(cls):
            if isinstance(value, AttributeImplDef):
                cls.attribute[value.signature] = value.impl
