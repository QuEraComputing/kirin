from kirin import ir


def test_hash():
    # Test hash collision for PyAttr with different values
    # This is a regression test for issue #1234
    # where hash(-1) == hash(-2) for PyAttr
    # This is fixed by using the value as bytes
    a = ir.PyAttr(-1)
    b = ir.PyAttr(-2)
    assert hash(a) != hash(b)
    assert hash(a.data) == hash(b.data)

    a = ir.PyAttr(-1.0)
    b = ir.PyAttr(-2.0)
    assert hash(a) != hash(b)
    assert hash(a.data) == hash(b.data)
