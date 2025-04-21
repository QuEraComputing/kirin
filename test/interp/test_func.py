import pytest

from kirin.prelude import basic_no_opt


def test_basic():
    @basic_no_opt
    def single(n):
        if n == 0:
            return 1
        else:
            return n

    assert single(0) == 1
    assert single(1) == 1
    assert single(2) == 2

    @basic_no_opt
    def single_2(n):
        if n == 0:
            n + 1  # type: ignore
        else:
            return n

    assert single_2(0) is None
    assert single_2(1) == 1
    assert single_2(2) == 2

    @basic_no_opt
    def single_3(n):
        if n == 0:
            n = n + 1
        else:
            n = n + 2

    assert single_3(0) is None
    assert single_3(1) is None

    @basic_no_opt
    def single_4(n):
        if n == 0:
            n = n + 1

    assert single_4(0) is None
    assert single_4(1) is None


def test_assert():

    @basic_no_opt
    def multi(n):
        if n == 0:
            return 1
        else:
            if n < 5:
                assert n < 2, "n must be less than 2"
            else:
                return n

    assert multi(0) == 1
    assert multi(1) is None

    with pytest.raises(AssertionError):
        multi(2)


def test_recursive():
    @basic_no_opt
    def factorial(n):
        if n == 0:
            return 1
        else:
            return n * factorial(n - 1)

    assert factorial(0) == 1
    assert factorial(1) == 1
    assert factorial(2) == 2
    assert factorial(3) == 6


def test_kw_call():
    @basic_no_opt
    def callee(n, m):
        return n, m

    @basic_no_opt
    def caller(n, m):
        return callee(m=m, n=n)

    assert caller(1, 2) == (1, 2)
