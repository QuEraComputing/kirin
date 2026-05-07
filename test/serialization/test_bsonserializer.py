from kirin.prelude import basic
from kirin.dialects import ilist
from kirin.serialization.bsonserializer import CompressedBSONSerializer


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
def foo2(y: int):

    def inner(x: int):
        return x * y + 1

    return inner


inner_ker = foo2(y=10)


@basic
def main_lambda(z: int):
    return inner_ker(z)


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
    encoded = basic.encode(program)
    decoded = basic.decode(encoded)
    assert decoded.code.is_structurally_equal(program.code)
    bson_serializer = CompressedBSONSerializer()
    bson_encoded = bson_serializer.encode(encoded)
    assert isinstance(bson_encoded, bytes)
    bson_decoded = bson_serializer.decode(bson_encoded)
    decoded_2 = basic.decode(bson_decoded)
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


def test_round_trip6():
    round_trip(main_lambda)


def test_deterministic():
    s1 = basic.encode(loop_ilist)
    bson_serializer = CompressedBSONSerializer()
    bson_s1 = bson_serializer.encode(s1)
    s2 = basic.encode(loop_ilist)
    bson_serializer2 = CompressedBSONSerializer()
    bson_s2 = bson_serializer2.encode(s2)
    assert bson_s1 == bson_s2


def test_eq():
    @basic
    def main():
        x = 0
        return x

    bson_serializer = CompressedBSONSerializer()
    bson_bytes = bson_serializer.encode(basic.encode(main))

    bson_serializer = CompressedBSONSerializer()
    main_des = basic.decode(bson_serializer.decode(bson_bytes))

    # without run_passes, we get errors in e.g. main == main_des
    assert hasattr(main_des, "run_passes")
