from abc import ABC
from dataclasses import dataclass
from typing import ClassVar, Generic, TypeVar

from kirin import ir
from kirin.codegen.base import CodeGen
from kirin.codegen.impl import ImplFunction, Signature

CodeGenType = TypeVar("CodeGenType", bound="CodeGen")
Target = TypeVar("Target")

@dataclass
class DialectEmit(ABC, Generic[CodeGenType, Target]):
    table: ClassVar[dict["Signature", "ImplFunction"]]

    @classmethod
    def fallback(cls, codegen: CodeGenType, stmt: "ir.Statement") -> Target: ...
