import inspect
from abc import ABC
from dataclasses import dataclass
from typing import TYPE_CHECKING, ClassVar

from kirin.codegen.impl import AttributeEmitDef, ImplDef

if TYPE_CHECKING:
    from kirin import ir
    from kirin.codegen.base import CodeGen
    from kirin.codegen.impl import ImplFunction, Signature


@dataclass
class DialectEmit(ABC):
    table: ClassVar[dict["Signature", "ImplFunction"]]

    @classmethod
    def fallback(cls, codegen: "CodeGen", stmt: "ir.Statement") -> None:
        raise NotImplementedError(f"Emit for {stmt.__class__} not implemented")

    def __init_subclass__(cls) -> None:
        super().__init_subclass__()
        cls.table = {}
        for _, value in inspect.getmembers(cls):
            if isinstance(value, ImplDef):
                for sig in value.signature:
                    cls.table[sig] = value.impl
            elif isinstance(value, AttributeEmitDef):
                cls.table[value.parent] = value.impl
