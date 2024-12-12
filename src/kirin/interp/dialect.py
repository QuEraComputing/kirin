import inspect
from abc import ABC
from typing import TYPE_CHECKING, Tuple, ClassVar
from dataclasses import dataclass

from kirin.ir import Statement
from kirin.exceptions import DialectInterpretationError
from kirin.interp.base import BaseInterpreter
from kirin.interp.impl import ImplDef
from kirin.interp.value import Result

if TYPE_CHECKING:
    from kirin.interp.base import BaseInterpreter
    from kirin.interp.impl import Signature, ImplFunction


@dataclass
class DialectInterpreter(ABC):
    """Base class to define lookup tables for interpreting code for IR nodes in a dialect."""

    table: ClassVar[dict["Signature", "ImplFunction"]]

    @classmethod
    def fallback(
        cls, interp: "BaseInterpreter", stmt: Statement, values: Tuple
    ) -> Result:
        raise DialectInterpretationError(
            f"Interpreter for {stmt.__class__} not implemented"
        )

    def __init_subclass__(cls) -> None:
        # init the subclass first
        super().__init_subclass__()
        cls.table = {}
        for _, value in inspect.getmembers(cls):
            if isinstance(value, ImplDef):
                for sig in value.signature:
                    cls.table[sig] = value.impl


@dataclass
class DefaultTypeInferInterpreter(DialectInterpreter):

    @classmethod
    def fallback(
        cls, interp: BaseInterpreter, stmt: Statement, values: Tuple
    ) -> Result:
        return tuple(result.type for result in stmt.results)


@dataclass
class EmptyDialectInterpreter(DialectInterpreter):
    pass
