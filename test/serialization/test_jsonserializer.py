from kirin.prelude import basic
from kirin.dialects import ilist
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
def loop_ilist():
    a = 0
    c = ilist.IList([a, a * 2])
    for i in range(3):
        a = i
        c = ilist.IList([a, a * 2])
    return c


@basic
def my_kernel1(x: int):
    return (x, x + 1, 3)


@basic
def my_kernel2(y: int):
    return my_kernel1(y) * 10


@basic
def slicing():
    in1 = ("a", "b", "c", "d", "e", "f", "g", "h")
    in2 = [1, 2, 3, 4, 5]

    x = slice(3, 5)
    a = in2[x]
    b = in1[1:4]
    c = in1[:3]
    d = in1[2:]
    e = in1[:]
    return (a, b, c, d, e)


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
    round_trip(loop_ilist)


def test_round_trip4():
    round_trip(my_kernel2)


def test_round_trip5():
    round_trip(slicing)


def test_deterministic():
    serializer = Serializer()
    s1 = serializer.encode(loop_ilist)
    json_serializer = JSONSerializer()
    json_s1 = json_serializer.encode(s1)
    serializer2 = Serializer()
    s2 = serializer2.encode(loop_ilist)
    json_serializer2 = JSONSerializer()
    json_s2 = json_serializer2.encode(s2)
    assert json_s1 == json_s2
