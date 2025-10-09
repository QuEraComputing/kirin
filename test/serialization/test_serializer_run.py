from kirin.prelude import basic
from kirin.dialects import ilist
from kirin.serialization.jsonserializer import JSONSerializer
from kirin.serialization.base.serializer import Serializer


@basic
def foo(x: int, y: float, z: bool):
    c = [[(200.0, 200.0), (210.0, 200.0)]]
    if z:
        c = [(222.0, 333.0)]
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


def test_round_trip_sequence_run():
    serializer = Serializer()
    encoded = serializer.encode(slicing)
    decoded = basic.decode(encoded)
    before = slicing()
    after = decoded()
    assert before == after
    # json_ser = JSONSerializer()
    # json_encoded = json_ser.encode(encoded)
    # json_decoded = json_ser.decode(json_encoded)
    # decoded_2 = basic.decode(json_decoded)
    # after2 = decoded_2(10)
    # assert before == after2 == after
test_round_trip_sequence_run()

def test_round_trip1_run():
    serializer = Serializer()
    encoded = serializer.encode(my_kernel1)
    decoded = basic.decode(encoded)
    before = my_kernel1(10)
    after = decoded(10)
    assert before == after
    json_ser = JSONSerializer()
    json_encoded = json_ser.encode(encoded)
    json_decoded = json_ser.decode(json_encoded)
    decoded_2 = basic.decode(json_decoded)
    after2 = decoded_2(10)
    assert before == after2 == after


def test_round_trip2_run():
    serializer = Serializer()
    encoded = serializer.encode(foo)
    decoded = basic.decode(encoded)
    before = foo(10, 20.0, True)
    after = decoded(10, 20.0, True)
    assert before == after
    json_ser = JSONSerializer()
    json_encoded = json_ser.encode(encoded)
    json_decoded = json_ser.decode(json_encoded)
    decoded_2 = basic.decode(json_decoded)
    after2 = decoded_2(10, 20.0, True)
    assert before == after2 == after
