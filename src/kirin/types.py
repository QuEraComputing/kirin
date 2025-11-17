"""Bindings for built-in types."""

import numbers

from kirin.ir.method import Method
from kirin.ir.attrs.types import (
    Union as Union,
    Vararg as Vararg,
    AnyType as AnyType,
    Generic as Generic,
    Literal as Literal,
    PyClass as PyClass,
    TypeVar as TypeVar,
    BottomType as BottomType,
    TypeAttribute as TypeAttribute,
    hint2type as hint2type,
    is_tuple_of as is_tuple_of,
    TypeofMethodType as TypeofMethodType,
    FunctionType as FunctionType,
)

Any = AnyType()
Bottom = BottomType()
Int = PyClass(int)
Float = PyClass(float)
Complex = PyClass(complex)
Number = PyClass(numbers.Number)
String = PyClass(str)
Bool = PyClass(bool)
NoneType = PyClass(type(None))
List = Generic(list, TypeVar("T"))
Slice = Generic(slice, TypeVar("T"))
Tuple = Generic(tuple, Vararg(TypeVar("T")))
Dict = Generic(dict, TypeVar("K"), TypeVar("V"))
Set = Generic(set, TypeVar("T"))
FrozenSet = Generic(frozenset, TypeVar("T"))
MethodType = TypeofMethodType()
