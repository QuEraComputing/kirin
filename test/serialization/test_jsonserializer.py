from kirin.prelude import basic
from kirin.serialization.jsonserializer import JSONSerializer
from kirin.serialization.base.serializer import Serializer
from kirin.serialization.base.deserializer import Deserializer


@basic
def foo(x: int, y: float, z: bool):
    c = [[(200.0, 200.0), (210.0, 200.0)]]
    if z:
        c.append([(222.0, 333.0)])
    else:
        return [1, 2, 3, 4]
    return c


@basic
def bar():
    def goo(x: int):
        a = (3, 4)
        return a[0]

    def boo(y):
        return goo(y) + 1

    boo(4)


@basic
def main():
    c = 0
    for i in range(3):
        c += i
    return c


def round_trip(program):
    serializer = Serializer()
    deserializer = Deserializer()
    encoded = serializer.encode(program)
    decoded = deserializer.decode(encoded)
    assert decoded.code.is_structurally_equal(program.code)
    json_serializer = JSONSerializer()
    json_encoded = json_serializer.encode(encoded)
    json_decoded = json_serializer.decode(json_encoded)
    decoded_2 = deserializer.decode(json_decoded)
    assert decoded_2.code.is_structurally_equal(program.code)


def test_round_trip1():
    round_trip(foo)


def test_round_trip2():
    round_trip(bar)


def test_round_trip3():
    round_trip(main)


def test_deterministic():
    serializer = Serializer()
    s1 = serializer.encode(main)
    json_serializer = JSONSerializer()
    json_s1 = json_serializer.encode(s1)
    serializer2 = Serializer()
    s2 = serializer2.encode(main)
    json_serializer2 = JSONSerializer()
    json_s2 = json_serializer2.encode(s2)
    assert json_s1 == json_s2
