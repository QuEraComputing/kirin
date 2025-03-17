"""Bindings for built-in types."""

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
)

Any = AnyType()
Bottom = BottomType()
Int = PyClass(int)
Float = PyClass(float)
Complex = PyClass(complex)
String = PyClass(str)
Bool = PyClass(bool)
NoneType = PyClass(type(None))
List = Generic(list, TypeVar("T"))
Slice = Generic(slice, TypeVar("T"))
Tuple = Generic(tuple, Vararg(TypeVar("T")))
Dict = Generic(dict, TypeVar("K"), TypeVar("V"))
Set = Generic(set, TypeVar("T"))
FrozenSet = Generic(frozenset, TypeVar("T"))
TypeofFunctionType = Generic[type(lambda: None)]
FunctionType = Generic(type(lambda: None), Tuple, Vararg(Any))
MethodType = Generic(Method, TypeVar("Params", Tuple), TypeVar("Ret"))
