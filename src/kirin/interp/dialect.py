import inspect
from abc import ABC
from typing import TYPE_CHECKING, TypeVar, ClassVar
from dataclasses import dataclass

from kirin.interp.base import BaseInterpreter
from kirin.interp.impl import ImplDef

if TYPE_CHECKING:
    from kirin.interp.base import BaseInterpreter
    from kirin.interp.impl import Signature, MethodFunction


InterpreterType = TypeVar("InterpreterType", bound="BaseInterpreter")
ValueType = TypeVar("ValueType")


@dataclass
class MethodTable(ABC):
    """Base class to define lookup tables for interpreting code for IR nodes in a dialect."""

    table: ClassVar[dict["Signature", "MethodFunction"]]

    def __init_subclass__(cls) -> None:
        # init the subclass first
        super().__init_subclass__()
        cls.table = {}
        for _, value in inspect.getmembers(cls):
            if isinstance(value, ImplDef):
                for sig in value.signature:
                    cls.table[sig] = value.impl
