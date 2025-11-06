from typing import Any

import numpy as np

from kirin.prelude import basic
from kirin.dialects import vmath
from kirin.dialects.vmath.passes import VMathDesugar
from kirin.dialects.ilist.runtime import IList


@basic.union([vmath])
def add_kernel(x, y):
    return x + y


@basic.union([vmath])(typeinfer=True)
def add_scalar_rhs_typed(x: IList[float, Any], y: float):
    return x + y


@basic.union([vmath])(aggressive=True, typeinfer=True)
def add_scalar_lhs():
    return add_kernel(x=3.0, y=[3.0, 4, 5])


def test_add_scalar_lhs():
    # out = add_scalar_lhs()
    VMathDesugar(add_scalar_lhs.dialects).unsafe_run(add_scalar_lhs)
    add_scalar_lhs.print()
    res = add_scalar_lhs()
    assert isinstance(res, IList)
    assert res.type.vars[0].typ is float
    assert np.allclose(np.asarray(res), np.array([6, 7, 8]))


def test_typed_kernel_add():
    VMathDesugar(add_scalar_rhs_typed.dialects).unsafe_run(add_scalar_rhs_typed)
    add_scalar_rhs_typed.print()
    res = add_scalar_rhs_typed(IList([0, 1, 2]), 3.1)
    assert np.allclose(np.asarray(res), np.asarray([3.1, 4.1, 5.1]))


@basic.union([vmath])
def add_two_lists():
    return add_kernel(x=[0, 1, 2], y=[3, 4, 5])


def test_add_lists():
    VMathDesugar(add_two_lists.dialects).unsafe_run(add_two_lists)
    res = add_two_lists()
    assert np.allclose(np.asarray(res), np.array([0, 1, 2, 3, 4, 5]))


@basic.union([vmath])
def sub_scalar_rhs_typed(x: IList[float, Any], y: float):
    return x - y


def test_sub_scalar_typed():
    VMathDesugar(sub_scalar_rhs_typed.dialects).unsafe_run(sub_scalar_rhs_typed)
    res = sub_scalar_rhs_typed(IList([0, 1, 2]), 3.1)
    assert np.allclose(np.asarray(res), np.asarray([-3.1, -2.1, -1.1]))


@basic.union([vmath])
def mult_scalar_lhs_typed(x: float, y: IList[float, Any]):
    return x * y


def test_mult_scalar_typed():
    VMathDesugar(mult_scalar_lhs_typed.dialects).unsafe_run(mult_scalar_lhs_typed)
    res = mult_scalar_lhs_typed(3, IList([0, 1, 2]))
    assert np.allclose(np.asarray(res), np.asarray([0, 3, 6]))


@basic.union([vmath])
def div_scalar_lhs_typed(x: float, y: IList[float, Any]):
    return x / y


def test_div_scalar_typed():
    VMathDesugar(div_scalar_lhs_typed.dialects).unsafe_run(div_scalar_lhs_typed)
    res = div_scalar_lhs_typed(3, IList([1, 1.5, 2]))
    assert np.allclose(np.asarray(res), np.asarray([3, 2, 1.5]))
