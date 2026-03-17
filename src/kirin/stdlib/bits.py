"""Bit-oriented helpers implemented as reusable Kirin kernels."""

from kirin.prelude import basic
from kirin.dialects import ilist


@basic
def _bit_length_rec(x: int, i: int) -> int:
    y = x >> i
    if y:
        return _bit_length_rec(x, i + 1)
    else:
        return i


@basic
def bit_length(x: int) -> int:
    """Return the number of bits required to represent ``x``."""
    x = abs(x)
    if x == 0:
        return 0
    return _bit_length_rec(x, 1)


@basic
def convert_bits(x: int, length: int):
    """Return the low ``length`` bits of ``x`` in least-significant-bit order. Note that the return type puts the least-significant-bit in the earliest index."""

    def _shift(i: int):
        return (x >> i) & 1

    return ilist.map(_shift, ilist.range(length))


__all__ = ["bit_length", "convert_bits"]
