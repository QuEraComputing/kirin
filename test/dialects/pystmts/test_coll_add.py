from kirin.ir import types
from kirin.prelude import basic


@basic(typeinfer=True)
def tuple_new(x: int, xs: tuple):
    return xs + (1, x)


@basic(typeinfer=True)
def list_new(x: int, xs: list):
    return xs + [1, x]


def test_tuple_add():
    assert tuple_new.return_type.is_subseteq(types.Tuple)
    assert list_new.return_type.is_subseteq(types.List)
