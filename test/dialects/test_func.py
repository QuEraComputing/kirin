import pytest

from kirin import ir
from kirin.prelude import structural_no_opt


def test_python_func():
    def some_func(x):
        return x + 1

    @structural_no_opt
    def dumm(x):
        return some_func(x)

    with pytest.raises(ir.TypeCheckError):
        dumm.code.verify_type()

    some_staff = ""

    @structural_no_opt
    def dumm2(x):
        return some_staff(x)  # type: ignore

    with pytest.raises(ir.TypeCheckError):
        dumm.code.verify_type()
