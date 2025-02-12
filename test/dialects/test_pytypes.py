import pytest

from kirin.types import (
    Int,
    Bool,
    Dict,
    Float,
    Slice,
    Tuple,
    Union,
    String,
    Vararg,
    AnyType,
    Literal,
    PyClass,
    TypeVar,
    NoneType,
    BottomType,
    TypeAttribute,
)


class Base:
    pass


class Derived(Base):
    pass


def test_union():
    assert PyClass(int) | PyClass(float) == Union(PyClass(int), PyClass(float))
    assert Union(PyClass(int), PyClass(int)) == PyClass(int)
    assert Union(PyClass(int), PyClass(float)) == Union(PyClass(int), PyClass(float))
    assert Union(Int, Float, BottomType()).is_equal(Union(Int, Float))
    assert hash(Union(PyClass(int), PyClass(float))) == hash(
        Union(PyClass(int), PyClass(float))
    )
    assert Union(PyClass(int), PyClass(float)) == Union(PyClass(float), PyClass(int))
    assert hash(Union(PyClass(int), PyClass(float))) == hash(
        Union(PyClass(float), PyClass(int))
    )
    assert Union(Union(Int, Float), BottomType()) == Union(Int, Float)
    assert Union(PyClass(int), AnyType()) == AnyType()
    assert Union(AnyType(), PyClass(int)) == AnyType()
    assert Union(BottomType(), PyClass(int)) == PyClass(int)
    assert Union(PyClass(int), BottomType()) == PyClass(int)
    assert PyClass(Derived).is_subseteq(PyClass(Base))
    assert Union(PyClass(Derived), PyClass(Base)) == PyClass(Base)
    assert AnyType() is AnyType()
    assert BottomType() is BottomType()
    t = Int.join(Float).join(String)
    assert t.is_subseteq(Int.join(Float).join(String))


def test_meet():
    assert PyClass(int).meet(PyClass(float)) == BottomType()
    assert PyClass(int).meet(PyClass(int)) == PyClass(int)
    assert PyClass(int).meet(AnyType()) == PyClass(int)
    assert AnyType().meet(PyClass(int)) == PyClass(int)
    assert BottomType().meet(PyClass(int)) == BottomType()
    assert PyClass(Base).meet(PyClass(Derived)) == PyClass(Derived)


def test_literal():
    assert Literal(Int) == Int
    assert Literal("aaa").join(Literal("bbb")) == Union(Literal("bbb"), Literal("aaa"))
    assert Literal("aaa").meet(Literal("bbb")) == BottomType()
    assert Literal("aaa").meet(Literal("aaa")) == Literal("aaa")
    assert Literal("aaa").is_subseteq(Literal("aaa") | String)
    assert Int.is_subseteq(Literal("aaa")) is False
    assert Tuple[Int].is_subseteq(Literal("aaa")) is False


def test_singleton():
    assert hash(AnyType()) == hash(AnyType())
    assert hash(AnyType()) == id(AnyType())
    assert hash(BottomType()) == hash(BottomType())
    assert hash(BottomType()) == id(BottomType())
    assert NoneType is NoneType
    assert Int is PyClass(int)
    assert Float is PyClass(float)
    assert String is PyClass(str)
    assert Bool is PyClass(bool)
    assert Literal("aaa") is Literal("aaa")


def test_generic_is_subseteq():
    assert Tuple[Literal("aaa")].is_subseteq(Tuple[Literal("aaa")])
    assert Tuple[Vararg(Int)][Int, Int] == Tuple[Int, Int]
    assert hash(Tuple[Int, Int]) == hash(Tuple[Int, Int])
    assert Tuple[Int, Vararg(Int)][Int, Int] == Tuple[Int, Int]
    assert Tuple[Int, Int].is_subseteq(Tuple[TypeVar("T"), Int])
    assert Dict[Int, Int].is_subseteq(Dict[TypeVar("K"), TypeVar("V")])
    assert Dict[Int, Int].is_subseteq(Dict)
    assert Dict[Int, Int].is_subseteq(Dict[Int])
    assert not Dict[Int, Int].is_subseteq(Dict[Float])
    assert PyClass(slice).is_subseteq(Slice)
    assert TypeVar("T", Int).is_subseteq(Int | String)

    with pytest.raises(TypeError):
        Tuple[Vararg(Int)][Int, Float]

    with pytest.raises(TypeError):
        Tuple[Vararg(Int), Int]


def test_generic_topbottom():
    t = Union(Int, Float)
    assert t.join(TypeAttribute.bottom()).is_subseteq(t)
    assert t.meet(TypeAttribute.bottom()).is_subseteq(TypeAttribute.bottom())
    assert t.join(TypeAttribute.top()).is_equal(TypeAttribute.top())
    assert t.meet(TypeAttribute.top()).is_equal(t)
