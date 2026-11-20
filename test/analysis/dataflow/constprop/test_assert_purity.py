from kirin.prelude import basic_no_opt
from kirin.analysis import const


@basic_no_opt
def checked_double(x: float):
    assert x > 0.0, "x must be positive"
    return x * 2.0


@basic_no_opt
def calls_checked_double():
    return checked_double(1.5)


def test_assert_known_true_keeps_frame_pure():
    frame, result = const.Propagate(basic_no_opt).run(checked_double, const.Value(1.5))

    assert result == const.Value(3.0)
    assert not frame.frame_is_not_pure


def test_assert_unknown_condition_is_impure():
    frame, _ = const.Propagate(basic_no_opt).run(checked_double, const.Unknown())

    assert frame.frame_is_not_pure


def test_assert_known_false_is_impure():
    frame, _ = const.Propagate(basic_no_opt).run(checked_double, const.Value(-1.0))

    assert frame.frame_is_not_pure


def test_call_with_passing_assert_is_pure():
    frame, result = const.Propagate(basic_no_opt).run(calls_checked_double)

    assert result == const.Value(3.0)
    assert not frame.frame_is_not_pure
