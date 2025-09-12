from kirin.prelude import basic
from kirin.serialization.binaryserializer import BinarySerializer


@basic
def foo():
    return 1


def test_to_json():
    # foo.code.print()
    serializer = BinarySerializer()
    encoded = serializer.encode(foo)
    decoded = serializer.decode(encoded)
    assert decoded.code.is_structurally_equal(foo.code)


test_to_json()
