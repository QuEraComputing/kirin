import pytest

from kirin.prelude import basic_no_opt


@basic_no_opt
def no_return(x):
    return


def test_noreturn():
    assert no_return(1) is None

    with pytest.raises(ValueError):
        no_return()


def test_noreturn_with_body():
    @basic_no_opt
    def no_return_with_body(x):
        x + 1

    assert no_return_with_body(1) is None
