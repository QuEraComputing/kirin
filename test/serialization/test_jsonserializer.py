from kirin.prelude import basic
from kirin.serialization.jsonserializer import JSONSerializer


@basic(typeinfer=True)
def foo(x: int, y: float, z: bool):
    c = [[(200.0, 200.0), (210.0, 200.0)]]
    if z:
        c.append([(222.0, 333.0)])
    return c


@basic(typeinfer=True)
def bar():
    def goo(x: int, y: int):
        return 42

    def boo(x, y):
        return goo(x, y)


@basic(typeinfer=True)
def main():
    c = 0
    for i in range(3):
        c += i
    return c


@basic(typeinfer=True)
def main2():
    return [1, 2, 3]


def test_to_json1():
    serializer = JSONSerializer()
    encoded = serializer.encode(foo)
    # print(encoded)
    decoded = serializer.decode(encoded)
    assert decoded.code.is_structurally_equal(foo.code)
    encoded = serializer.encode_to_str(foo)
    decoded = serializer.decode_from_str(encoded)
    assert decoded.code.is_structurally_equal(foo.code)


def test_to_json2():
    serializer = JSONSerializer()
    encoded = serializer.encode(bar)
    decoded = serializer.decode(encoded)
    assert decoded.code.is_structurally_equal(bar.code)
    encoded = serializer.encode_to_str(bar)
    decoded = serializer.decode_from_str(encoded)
    assert decoded.code.is_structurally_equal(bar.code)


def test_to_json3():
    serializer = JSONSerializer()
    encoded = serializer.encode(main)
    decoded = serializer.decode(encoded)
    assert decoded.code.is_structurally_equal(main.code)
    encoded = serializer.encode_to_str(main)
    decoded = serializer.decode_from_str(encoded)
    assert decoded.code.is_structurally_equal(main.code)


def test_to_json4():
    serializer = JSONSerializer()
    encoded = serializer.encode(main2)
    decoded = serializer.decode(encoded)
    assert decoded.code.is_structurally_equal(main2.code)
    encoded = serializer.encode_to_str(main2)
    decoded = serializer.decode_from_str(encoded)
    assert decoded.code.is_structurally_equal(main2.code)


def test_deterministic():
    serializer = JSONSerializer()
    s1 = serializer.encode_to_str(main)
    serializer2 = JSONSerializer()
    s2 = serializer2.encode_to_str(main)
    assert s1 == s2
