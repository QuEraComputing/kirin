from kirin.prelude import basic
from kirin.dialects.py import types

@basic
def tuple_hint(xs: tuple[int, ...]):
    types.Tuple[types.Int]


def test_tuple_hint():
    tuple_hint.arg_types[0].is_subtype()
