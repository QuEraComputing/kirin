from kirin.dialects.fcf import Foldl, Foldr, MapList, Scan
from kirin.dialects.py import types
from kirin.prelude import basic


@basic
def add(x: int, y: int):
    return x + y


@basic
def add1(x: int):
    return x + 1.0


@basic
def add2(x: int, y: int):
    return x + y, y


@basic(typeinfer=True)
def foldl(xs: list[int]):
    return Foldl(add, xs, 0)


@basic(typeinfer=True)
def foldr(xs: list[int]):
    return Foldr(add, xs, 0)


@basic(typeinfer=True)
def map_list(xs: list[int]):
    return MapList(add1, xs)


@basic(typeinfer=True)
def scan(xs: list[int]):
    return Scan(add2, 0, xs)


def test_fold():
    xs = [1, 2, 3, 4, 5]
    assert foldl(xs) == sum(xs)
    assert foldr(xs) == sum(xs)
    assert map_list.return_type.is_subtype(types.List[types.Float])
    assert map_list([1, 2, 3]) == [2.0, 3.0, 4.0]
    assert scan([1, 2, 3, 4, 5]) == (15, [1, 2, 3, 4, 5])
    assert scan.return_type.is_subseteq(types.Tuple[types.Int, types.List[types.Int]])
