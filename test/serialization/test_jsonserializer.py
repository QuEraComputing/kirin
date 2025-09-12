from kirin.prelude import basic
from kirin.serialization.jsonserializer import JSONSerializer


@basic
def foo(x: int, y: float):
    c = [[(200.0, 200.0), (210.0, 200.0)]]
    return c


@basic
def bar():
    def goo(x: int, y: int):
        return 42

    def foo(x: str, y: str):
        return goo(1, 2)

    return foo("hello", "world")


@basic
def main():
    for i in range(3):
        foo(i, 0.1)


def test_to_json1():
    # foo.code.print()
    serializer = JSONSerializer()
    encoded = serializer.encode(foo)
    # foo.code.print()
    # print(encoded)
    decoded = serializer.decode(encoded)
    # decoded.code.print()
    assert decoded.code.is_structurally_equal(foo.code)


def test_to_json2():
    # bar.code.print()
    serializer = JSONSerializer()
    encoded = serializer.encode(bar)
    # bar.code.print()
    # print(encoded)
    decoded = serializer.decode(encoded)
    # decoded.code.print()
    assert decoded.code.is_structurally_equal(bar.code)


def test_to_json3():
    # main.code.print()
    serializer = JSONSerializer()
    encoded = serializer.encode(main)
    # main.code.print()
    # print(encoded)
    decoded = serializer.decode(encoded)
    # decoded.code.print()
    assert decoded.code.is_structurally_equal(main.code)


test_to_json1()
test_to_json2()
test_to_json3()
