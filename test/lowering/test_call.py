import pytest

from kirin.prelude import python_no_opt
from kirin.dialects import cf, fcf, func
from kirin.lowering import Lowering
from kirin.exceptions import DialectLoweringError

lowering = Lowering(python_no_opt.union([cf, func]))


def add(n):
    return n + 1


def unknown_stmt():
    fcf.Map(add, [1, 2, 3])


def test_unknown_stmt():
    with pytest.raises(DialectLoweringError):
        lowering.run(unknown_stmt)
