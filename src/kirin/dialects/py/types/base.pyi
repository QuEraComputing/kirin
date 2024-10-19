from dataclasses import dataclass

from kirin.ir import TypeAttribute

from .elem import (
    PyAnyType,
    PyBottomType,
    PyClass,
    PyConst,
    PyGeneric,
    PyLiteral,
    PyTypeVar,
    PyUnion,
)

@dataclass
class _PyType(TypeAttribute):
    @classmethod
    def top(cls) -> PyAnyType: ...
    @classmethod
    def bottom(cls) -> PyBottomType: ...
    def is_subseteq(self, other: TypeAttribute) -> bool: ...
    def is_subseteq_PyAnyType(self, other: PyAnyType) -> bool: ...
    def is_subseteq_PyBottomType(self, other: PyBottomType) -> bool: ...
    def is_subseteq_PyUnion(self, other: PyUnion) -> bool: ...
    def is_subseteq_PyLiteral(self, other: PyLiteral) -> bool: ...
    def is_subseteq_PyTypeVar(self, other: PyTypeVar) -> bool: ...
    def is_subseteq_PyConst(self, other: PyConst) -> bool: ...
    def is_subseteq_PyClass(self, other: PyClass) -> bool: ...
    def is_subseteq_PyGeneric(self, other: PyGeneric) -> bool: ...
    def is_subseteq_fallback(self, other: TypeAttribute) -> bool: ...
