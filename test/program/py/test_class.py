import pytest

from kirin.prelude import basic


class Foo:

    def some_kernel(self):
        @basic(verify=False)
        def goo(x: int):
            return x

        return goo

    def another_kernel(self):
        @basic(verify=False)
        def goo(x: int):
            kernel = self.some_kernel()
            kernel(x)

        return goo


def test_call_method_error():
    foo = Foo()
    goo = foo.another_kernel()

    with pytest.raises(Exception):
        goo.verify_type()
