from kirin.prelude import basic
from kirin.serialization.jsonserializer import JSONSerializer


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
    json_serializer = JSONSerializer()
    encoded = json_serializer.encode(program)
    decoded = json_serializer.decode(encoded)
    # program.code.print()
    # print()
    # decoded.code.print()
    assert decoded.code.is_structurally_equal(program.code)
    # encoded = json_serializer.encode_to_str(program)
    # decoded = json_serializer.decode_from_str(encoded)
    # assert decoded.code.is_structurally_equal(program.code)


def test_round_trip1():
    round_trip(foo)


def test_round_trip2():
    round_trip(bar)


def test_round_trip3():
    round_trip(main)


def test_deterministic():
    json_serializer = JSONSerializer()
    s1 = json_serializer.encode_to_str(main)
    json_serializer2 = JSONSerializer()
    s2 = json_serializer2.encode_to_str(main)
    assert s1 == s2


test_deterministic()
test_round_trip1()
test_round_trip2()
test_round_trip3()
