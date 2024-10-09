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
def tuple_vararg_slice(xs: tuple[int, ...], i: slice):
    return xs[i]


@basic(typeinfer=True)
def tuple_const_slice(xs: tuple[int, float, str]):
    return xs[1:]


@basic(typeinfer=True)
def tuple_const_index(xs: tuple[int, float, str]):
    return xs[1]


@basic(typeinfer=True)
def tuple_err(xs: tuple[int, float, str], i: str):
    return xs[i]


@basic(typeinfer=True)
def tuple_const_err(xs: tuple[int, float, str]):
    return xs[3]


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
    assert tuple_const_index.return_type.is_subseteq(types.Float)
    assert tuple_vararg_slice.return_type.is_subseteq(
        types.Tuple[types.PyVararg(types.Int)]
    )
    assert tuple_const_slice.return_type.is_subseteq(
        types.Tuple[types.Float, types.String]
    )
    assert tuple_err.return_type == types.Bottom
    assert tuple_const_err.return_type == types.Bottom
    assert list_infer.return_type.is_subseteq(types.Int)
    assert list_slice.return_type.is_subseteq(types.List[types.Int])
    assert unknown.return_type == types.Any
