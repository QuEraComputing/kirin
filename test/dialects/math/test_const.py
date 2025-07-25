import math as pymath

from kirin.dialects import math


def test_const():
    assert math.pi == pymath.pi
    assert math.e == pymath.e
    assert math.tau == pymath.tau
