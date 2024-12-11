from kirin import types
from kirin.prelude import basic


@basic
def tuple_hint(xs: tuple[int, ...]):
    types.Tuple[types.Int]


def test_tuple_hint():
    assert tuple_hint.arg_types[0].is_subseteq(types.Tuple[types.Vararg(types.Int)])
