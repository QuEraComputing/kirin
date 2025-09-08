from kirin.prelude import basic
from kirin.serialization.jsonserializer import JSONSerializer


@basic
def foo(x: int, y: float):
    return x + y * 2


def test_to_json1():
    # foo.code.print()
    serializer = JSONSerializer()
    encoded = serializer.encode(foo)
    decoded = serializer.decode(encoded)
    # encoded_again = serializer.encode(decoded)
    # print(encoded)
    assert decoded.code.is_structurally_equal(foo.code)


test_to_json1()
