from kirin.prelude import basic


@basic
def foo(x: int):  # type: ignore
    def goo(y: int):
        return x + y

    return goo


@basic
def main(y: int):
    x = 1
    f = foo(x)
    return f(y)


def test_main():
    assert main(1) == 2
    assert main(2) == 3
