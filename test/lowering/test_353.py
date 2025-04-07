import typing

from kirin import types
from kirin.prelude import basic

N = typing.TypeVar("N")


class MyTest(typing.Generic[N]):
    pass


def test_generic_type_hint():
    @basic
    def test(obj: MyTest[N]):
        return None

    assert test.arg_types[0].is_subseteq(types.PyClass(MyTest))
