import pytest

from kirin.dialects.py.types.builtin import (
    Bool,
    Dict,
    Float,
    Int,
    List,
    NoneType,
    Slice,
    String,
    Tuple,
)
from kirin.dialects.py.types.elem import (
    PyAnyType,
    PyBottomType,
    PyClass,
    PyLiteral,
    PyTypeVar,
    PyUnion,
    PyVararg,
)
from kirin.ir import TypeAttribute


class Base:
    pass


class Derived(Base):
    pass


def test_union():
    assert PyClass(int) | PyClass(float) == PyUnion(PyClass(int), PyClass(float))
    assert PyUnion(PyClass(int), PyClass(int)) == PyClass(int)
    assert PyUnion(PyClass(int), PyClass(float)) == PyUnion(
        PyClass(int), PyClass(float)
    )
    assert PyUnion(Int, Float, PyBottomType()).is_equal(PyUnion(Int, Float))
    assert hash(PyUnion(PyClass(int), PyClass(float))) == hash(
        PyUnion(PyClass(int), PyClass(float))
    )
    assert PyUnion(PyClass(int), PyClass(float)) == PyUnion(
        PyClass(float), PyClass(int)
    )
    assert hash(PyUnion(PyClass(int), PyClass(float))) == hash(
        PyUnion(PyClass(float), PyClass(int))
    )
    assert PyUnion(PyUnion(Int, Float), PyBottomType()) == PyUnion(Int, Float)
    assert PyUnion(PyClass(int), PyAnyType()) == PyAnyType()
    assert PyUnion(PyAnyType(), PyClass(int)) == PyAnyType()
    assert PyUnion(PyBottomType(), PyClass(int)) == PyClass(int)
    assert PyUnion(PyClass(int), PyBottomType()) == PyClass(int)
    assert PyClass(Derived).is_subtype(PyClass(Base))
    assert PyUnion(PyClass(Derived), PyClass(Base)) == PyClass(Base)
    assert PyAnyType() is PyAnyType()
    assert PyBottomType() is PyBottomType()
    t = Int.join(Float).join(String)
    assert t.is_subseteq(Int.join(Float).join(String))


def test_meet():
    assert PyClass(int).meet(PyClass(float)) == PyBottomType()
    assert PyClass(int).meet(PyClass(int)) == PyClass(int)
    assert PyClass(int).meet(PyAnyType()) == PyClass(int)
    assert PyAnyType().meet(PyClass(int)) == PyClass(int)
    assert PyBottomType().meet(PyClass(int)) == PyBottomType()
    assert PyClass(Base).meet(PyClass(Derived)) == PyClass(Derived)


def test_literal():
    assert PyLiteral(Int) == Int
    assert PyLiteral("aaa").join(PyLiteral("bbb")) == PyUnion(
        PyLiteral("bbb"), PyLiteral("aaa")
    )
    assert PyLiteral("aaa").meet(PyLiteral("bbb")) == PyBottomType()
    assert PyLiteral("aaa").meet(PyLiteral("aaa")) == PyLiteral("aaa")
    assert PyLiteral("aaa").is_subseteq(PyLiteral("aaa") | String)
    assert Int.is_subseteq(PyLiteral("aaa")) is False
    assert Tuple[Int].is_subseteq(PyLiteral("aaa")) is False


def test_singleton():
    assert hash(PyAnyType()) == hash(PyAnyType())
    assert hash(PyAnyType()) == id(PyAnyType())
    assert hash(PyBottomType()) == hash(PyBottomType())
    assert hash(PyBottomType()) == id(PyBottomType())
    assert NoneType is NoneType
    assert Int is PyClass(int)
    assert Float is PyClass(float)
    assert String is PyClass(str)
    assert Bool is PyClass(bool)
    assert PyLiteral("aaa") is PyLiteral("aaa")


def test_generic_is_subtype():
    assert Tuple[PyLiteral("aaa")].is_subseteq(Tuple[PyLiteral("aaa")])
    assert Tuple[PyVararg(Int)][Int, Int] == Tuple[Int, Int]
    assert hash(Tuple[Int, Int]) == hash(Tuple[Int, Int])
    assert Tuple[Int, PyVararg(Int)][Int, Int] == Tuple[Int, Int]
    assert List[Int] == List[Int]
    assert hash(List[Int]) == hash(List[Int])
    assert Tuple[Int, Int].is_subtype(Tuple[PyTypeVar("T"), Int])
    assert Dict[Int, Int].is_subtype(Dict[PyTypeVar("K"), PyTypeVar("V")])
    assert Dict[Int, Int].is_subtype(Dict)
    assert Dict[Int, Int].is_subtype(Dict[Int])
    assert not Dict[Int, Int].is_subtype(Dict[Float])
    assert PyClass(slice).is_subseteq(Slice)
    assert PyTypeVar("T", Int).is_subseteq(Int | String)

    with pytest.raises(TypeError):
        Tuple[PyVararg(Int)][Int, Float]

    with pytest.raises(TypeError):
        Tuple[PyVararg(Int), Int]

    with pytest.raises(ValueError):
        PyTypeVar("T").is_subtype(PyTypeVar("T"))


def test_generic_topbottom():
    t = PyUnion(Int, Float)
    assert t.join(TypeAttribute.bottom()).is_subseteq(t)
    assert t.meet(TypeAttribute.bottom()).is_subseteq(TypeAttribute.bottom())
    assert t.join(TypeAttribute.top()).is_equal(TypeAttribute.top())
    assert t.meet(TypeAttribute.top()).is_equal(t)
