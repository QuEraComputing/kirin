from kirin import ir
from kirin.prelude import basic
from kirin.dialects.py.binop.stmts import Add, Mult
from kirin.serialization.binaryserializer import BinarySerializer


@basic
def foo():
    return 1


def test_to_json():
    # foo.code.print()
    dialect = ir.Dialect(name="math", stmts=[Add, Mult])
    serializer = BinarySerializer(dialects=dialect)
    encoded = serializer.encode(foo)
    decoded = serializer.decode(encoded)
    # encoded_again = serializer.encode(decoded)
    # decoded.code.print()

    assert decoded.code.is_structurally_equal(foo.code)


test_to_json()
