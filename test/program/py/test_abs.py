import pytest

from kirin.prelude import basic_no_opt
from kirin.dialects.py import builtin


@basic_no_opt
def abs_kernel(x):
    return builtin.Abs(value=x)


@pytest.mark.parametrize("x", [-1, 2, 3.0, -3.2])
def test_abs(x):

    abs_x = abs_kernel(x)

    assert isinstance(abs_x, type(abs(x)))
    assert abs_x == abs(x)
