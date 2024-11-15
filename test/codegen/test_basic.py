from kirin.codegen import DictGen
from kirin.prelude import basic


@basic
def foo(x: int):  # type: ignore
    def goo(y: int):
        return x + y

    return goo


def test_basic():
    d = DictGen(basic).emit(foo)
    print(d)
