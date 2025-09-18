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


def test_round_trip1():
    json_serializer = JSONSerializer()
    encoded = json_serializer.encode(foo)
    decoded = json_serializer.decode(encoded)
    assert decoded.code.is_structurally_equal(foo.code)
    encoded = json_serializer.encode_to_str(foo)
    decoded = json_serializer.decode_from_str(encoded)
    assert decoded.code.is_structurally_equal(foo.code)


def test_round_trip2():
    json_serializer = JSONSerializer()
    encoded = json_serializer.encode(bar)
    decoded = json_serializer.decode(encoded)
    assert decoded.code.is_structurally_equal(bar.code)
    encoded = json_serializer.encode_to_str(bar)
    decoded = json_serializer.decode_from_str(encoded)
    assert decoded.code.is_structurally_equal(bar.code)


def test_round_trip3():
    json_serializer = JSONSerializer()
    encoded = json_serializer.encode(main)
    decoded = json_serializer.decode(encoded)
    assert decoded.code.is_structurally_equal(main.code)
    encoded = json_serializer.encode_to_str(main)
    decoded = json_serializer.decode_from_str(encoded)
    assert decoded.code.is_structurally_equal(main.code)


def test_round_trip4():
    json_serializer = JSONSerializer()
    encoded = json_serializer.encode(main2)
    decoded = json_serializer.decode(encoded)
    assert decoded.code.is_structurally_equal(main2.code)
    encoded = json_serializer.encode_to_str(main2)
    decoded = json_serializer.decode_from_str(encoded)
    assert decoded.code.is_structurally_equal(main2.code)


def test_deterministic():
    json_serializer = JSONSerializer()
    s1 = json_serializer.encode_to_str(main)
    json_serializer2 = JSONSerializer()
    s2 = json_serializer2.encode_to_str(main)
    assert s1 == s2
