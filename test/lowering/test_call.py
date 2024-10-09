import pytest

from kirin.dialects import cf, fcf, func
from kirin.dialects.py import data, stmts, types
from kirin.exceptions import DialectLoweringError
from kirin.lowering import Lowering

lowering = Lowering([cf, func, stmts, data, types])


def add(n):
    return n + 1


def unknown_stmt():
    fcf.Map(add, [1, 2, 3])


def test_unknown_stmt():
    with pytest.raises(DialectLoweringError):
        lowering.run(unknown_stmt)
