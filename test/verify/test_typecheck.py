import pytest

from kirin.prelude import basic
from kirin.dialects import math
from kirin.exceptions import TypeCheckError


@basic(verify=False, typeinfer=False)
def check_type_err(a, b):
    math.sin(a)
    return math.sin(b)


def test_check_type():
    with pytest.raises(TypeCheckError):
        check_type_err.code.verify_type()
