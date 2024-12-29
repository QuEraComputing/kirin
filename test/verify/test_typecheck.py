import pytest

from kirin.prelude import basic
from kirin.dialects import math
from kirin.exceptions import VerificationError


@basic(verify=False, typeinfer=False)
def typecheck_err(a, b):
    math.sin(a)
    return math.sin(b)


def test_typecheck():
    with pytest.raises(VerificationError):
        typecheck_err.code.typecheck()
