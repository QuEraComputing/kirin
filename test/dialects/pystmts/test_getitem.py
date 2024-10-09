from kirin.dialects.py import types
from kirin.prelude import basic


@basic(typeinfer=True)
def tuple_vararg(xs: tuple[int, ...], i: int):
    return xs[i]


@basic(typeinfer=True)
def tuple_multi(xs: tuple[int, float, str], i: int):
    return xs[i]


@basic(typeinfer=True)
def tuple_slice(xs: tuple[int, float, str], i: slice):
    return xs[i]


@basic(typeinfer=True)
def list_infer(xs: list[int], i: int):
    return xs[i]


@basic(typeinfer=True)
def list_slice(xs: list[int], i: slice):
    return xs[i]


@basic(typeinfer=True)
def unknown(xs, i: int):
    return xs[i]


def test_getitem_typeinfer():
    assert tuple_vararg.return_type.is_subseteq(types.Int)
    assert tuple_multi.return_type.is_subseteq(types.Int | types.Float | types.String)
    assert tuple_slice.return_type.is_subseteq(
        types.Tuple[types.PyVararg(types.Int | types.Float | types.String)]
    )
    assert list_infer.return_type.is_subseteq(types.Int)
    assert list_slice.return_type.is_subseteq(types.List[types.Int])
    assert unknown.return_type == types.Any
