from kirin import ir
from kirin.prelude import basic
from kirin.dialects import ilist


def test_lambda_comp_with_closure():
    @basic(fold=False)
    def main(z, r):
        return (lambda x: x + z)(r)

    assert main(3, 4) == 7


def test_lambda_comp():
    @basic(fold=False)
    def main(z):
        return lambda x: x + z

    x = main(3)
    assert isinstance(x, ir.Method)
    assert x(4) == 7


def test_invoke_from_lambda_comp():

    @basic
    def foo(a):
        return a * 2

    @basic(fold=False)
    def main(z):
        return lambda x: x + foo(z)

    x = main(3)

    assert isinstance(x, ir.Method)
    assert x(4) == 10


def test_lambda_in_lambda():

    @basic(fold=False)
    def main(z):

        def my_foo(a):
            return lambda x: x * a

        return my_foo(z)

    x = main(3)

    assert isinstance(x, ir.Method)
    assert x(4) == 12


def test_ilist_map():

    @basic(fold=False)
    def main(z):
        return ilist.map(lambda x: x + z, ilist.range(10))

    x = main(3)
    assert len(x) == 10
    assert x.data == [3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
