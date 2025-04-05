import pytest

from kirin import ir
from kirin.prelude import basic
from kirin.dialects import math


@basic(verify=False, typeinfer=False)
def check_type_err(a, b):
    math.sin(a)
    return math.sin(b)


def test_check_type():
    with pytest.raises(ir.TypeCheckError):
        check_type_err.code.verify_type()
