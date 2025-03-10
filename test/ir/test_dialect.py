import pytest

from kirin.ir import Dialect
from kirin.types import PyClass


def test_py_type_register():

    class TestClass:
        pass

    class OtherTestClass:
        pass

    dialect = Dialect("test")

    TestType = dialect.register_py_type(
        TestClass, display_name="TestType", prefix="test"
    )
    assert TestType == PyClass(TestClass, prefix="test", display_name="TestType")

    assert dialect.python_types == {("test", "TestType"): TestType}

    with pytest.raises(ValueError):
        dialect.register_py_type(OtherTestClass, display_name="TestType", prefix="test")

    with pytest.raises(ValueError):
        dialect.register_py_type(TestClass, display_name="TestClass", prefix="test")

    with pytest.raises(ValueError):
        dialect.register_py_type(
            TestClass, display_name="TestType", prefix="other_prefix"
        )
