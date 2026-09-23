from kirin import types
from kirin.prelude import basic
from kirin.analysis import TypeInference


@basic(typeinfer=True)
def int_plus_bool(a: int, flag: bool):
    return a + flag


@basic(typeinfer=True)
def bool_plus_bool(flag: bool, other: bool):
    return flag + other


@basic(typeinfer=True)
def quotient_plus_bool(a: int, b: int, flag: bool):
    quotient = a // b
    return quotient + flag


@basic(typeinfer=True)
def float_times_int(x: float, n: int):
    return x * n


@basic(typeinfer=True)
def bool_and_bool(flag: bool, other: bool):
    return flag & other


@basic(typeinfer=True)
def int_and_bool(a: int, flag: bool):
    return a & flag


def test_arithmetic_on_a_bool_gives_an_int():
    assert int_plus_bool.return_type == types.Int
    assert bool_plus_bool.return_type == types.Int
    assert quotient_plus_bool.return_type == types.Int
    stmts = list(quotient_plus_bool.callable_region.walk())
    quotient = next(s for s in stmts if s.name == "floordiv")
    assert quotient.results[0].type == types.Int


def test_arithmetic_with_a_float_gives_a_float():
    assert float_times_int.return_type == types.Float


def test_bitwise_keeps_two_bools_and_widens_a_mix():
    assert bool_and_bool.return_type == types.Bool
    assert int_and_bool.return_type == types.Int


def test_a_bottom_operand_gives_bottom():
    inference = TypeInference(basic)
    _, result = inference.run(int_plus_bool, types.Bottom, types.Bool)
    assert result == types.Bottom
