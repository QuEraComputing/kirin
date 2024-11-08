import inspect
from abc import ABC
from dataclasses import dataclass
from typing import TYPE_CHECKING, ClassVar, Tuple

from kirin.exceptions import DialectInterpretationError
from kirin.interp.base import BaseInterpreter
from kirin.interp.impl import ImplDef
from kirin.interp.value import Result, ResultValue
from kirin.ir import Statement

if TYPE_CHECKING:
    from kirin.interp.base import BaseInterpreter
    from kirin.interp.impl import ImplFunction, Signature


@dataclass
class DialectInterpreter(ABC):
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
        return ResultValue(*tuple(result.type for result in stmt.results))


@dataclass
class EmptyDialectInterpreter(DialectInterpreter):
    pass
