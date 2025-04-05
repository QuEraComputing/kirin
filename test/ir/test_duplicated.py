import pytest

from kirin import ir
from kirin.prelude import basic


def test_main():
    y = 1

    @basic
    def foo(x):
        return x + y

    with pytest.raises(ir.CompilerError):

        @basic
        def foo(x):  # noqa: F811
            return x + y
