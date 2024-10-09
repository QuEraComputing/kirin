"""some convenient singleton wrapper for python.

They should be equivalent directly construct PyClass.
"""

from kirin.dialects.py.types.base import (
    PyAnyType,
    PyBottomType,
    PyClass,
    PyGeneric,
    PyTypeVar,
    PyVararg,
)

Any = PyAnyType()
Bottom = PyBottomType()
Int = PyClass(int)
Float = PyClass(float)
String = PyClass(str)
Bool = PyClass(bool)
NoneType = PyClass(type(None))
Slice = PyGeneric(slice, PyTypeVar("T"))
List = PyGeneric(list, PyTypeVar("T"))
Tuple = PyGeneric(tuple, PyVararg(PyTypeVar("T")))
Dict = PyGeneric(dict, PyTypeVar("K"), PyTypeVar("V"))
Set = PyGeneric(set, PyTypeVar("T"))
FrozenSet = PyGeneric(frozenset, PyTypeVar("T"))
TypeofFunctionType = PyGeneric[type(lambda: None)]
FunctionType = PyGeneric(type(lambda: None), Tuple, PyVararg(PyAnyType()))
