from kirin import types
from kirin.prelude import basic
from kirin.dialects.ilist import IList, IListType


@basic(typeinfer=True)
def tuple_new(x: int, xs: tuple):
    return xs + (1, x)


@basic(typeinfer=True)
def list_new(x: int, xs: IList):
    return xs + [1, x]


def test_tuple_add():
    assert tuple_new.return_type.is_subseteq(types.Tuple)
    assert list_new.return_type.is_subseteq(IListType)
