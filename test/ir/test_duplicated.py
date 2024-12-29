import pytest

from kirin.prelude import basic
from kirin.exceptions import CompilerError


def test_main():
    y = 1

    @basic
    def foo(x):
        return x + y

    with pytest.raises(CompilerError):

        @basic
        def foo(x):  # noqa: F811
            return x + y
