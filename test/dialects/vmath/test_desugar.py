# from typing import Any
#
# import numpy as np
#
# from kirin import types
from kirin.prelude import basic
from kirin.dialects import vmath


@basic.union([vmath])
def add_kernel(x, y):
    return x + y


@basic.union([vmath])
def add_two_lists():
    return add_kernel(x=[0, 1, 2], y=[3, 4, 5])


@basic.union([vmath])(aggressive=True)
def add_scalar_lhs():
    return add_kernel(x=3.0, y=[3.0, 4, 5])


def test_add_scalar_lhs():
    # out = add_scalar_lhs()
    import ipdb

    ipdb.set_trace()
