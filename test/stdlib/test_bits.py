from kirin.dialects import ilist
from kirin.stdlib.bits import bit_length, convert_bits


def test_bit_length_negative():
    assert bit_length(-13) == 4


def test_bit_length_zero():
    assert bit_length(0) == 0


def test_bit_length_positive():
    assert bit_length(7) == 3


def test_bit_length_large():
    x = (1 << 80) + 12345
    assert bit_length(x) == x.bit_length()


def test_bit_length_large_power_of_two():
    x = 1 << 80
    assert bit_length(x) == 81


def test_bit_length_small():
    assert bit_length(3) == 2


def test_bit_length_small_single_bit():
    assert bit_length(1) == 1


def test_convert_bits_length_greater_than_bit_length():
    out = convert_bits(5, 5)
    assert isinstance(out, ilist.IList)
    assert out.data == [1, 0, 1, 0, 0]


def test_convert_bits_length_equal_to_bit_length():
    out = convert_bits(5, 3)
    assert isinstance(out, ilist.IList)
    assert out.data == [1, 0, 1]


def test_convert_bits_length_less_than_bit_length():
    out = convert_bits(13, 2)
    assert isinstance(out, ilist.IList)
    assert out.data == [1, 0]


def test_convert_bits_negative_x():
    out = convert_bits(-1, 4)
    assert isinstance(out, ilist.IList)
    assert out.data == [1, 1, 1, 1]


def test_convert_bits_negative_length():
    out = convert_bits(5, -3)
    assert isinstance(out, ilist.IList)
    assert out.data == []


def test_convert_bits_zero_x():
    out = convert_bits(0, 4)
    assert isinstance(out, ilist.IList)
    assert out.data == [0, 0, 0, 0]


def test_convert_bits_zero_length():
    out = convert_bits(7, 0)
    assert isinstance(out, ilist.IList)
    assert out.data == []


def test_convert_bits_small_x():
    out = convert_bits(2, 3)
    assert isinstance(out, ilist.IList)
    assert out.data == [0, 1, 0]


def test_convert_bits_small_length():
    out = convert_bits(7, 1)
    assert isinstance(out, ilist.IList)
    assert out.data == [1]


def test_convert_bits_large_x():
    x = (1 << 12) + (1 << 5) + 1
    out = convert_bits(x, 13)
    assert isinstance(out, ilist.IList)
    assert out.data == [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1]


def test_convert_bits_large_length():
    out = convert_bits(3, 10)
    assert isinstance(out, ilist.IList)
    assert out.data == [1, 1, 0, 0, 0, 0, 0, 0, 0, 0]
