import pytest

from kirin.testing import assert_structurally_same
from kirin.prelude import structural_no_opt
from kirin.dialects import py, func


@pytest.fixture
def simple_example():
    """Kernel that computes 1*1 + 1."""

    @structural_no_opt
    def main():
        x = 1
        y = x * x
        z = y + x
        return z

    return main


def test_correct_list_passes_and_prints_nothing(simple_example, capfd):
    main = simple_example

    expected_statements = [
        x := py.Constant(1),
        y := py.Mult(x.result, x.result),
        z := py.Add(y.result, x.result),
        func.Return(z.result),
    ]
    assert_structurally_same(main, expected_statements)
    out, err = capfd.readouterr()
    assert out == ""
    assert err == ""


def test_debug_true_prints_something(simple_example, capfd):
    main = simple_example

    expected_statements = [
        x := py.Constant(1),
        y := py.Mult(x.result, x.result),
        z := py.Add(y.result, x.result),
        func.Return(z.result),
    ]
    assert_structurally_same(main, expected_statements, debug=True)
    out, err = capfd.readouterr()
    assert len(out.strip().split("\n")) == 4
    assert err == ""


def test_wrong_length_fails(simple_example):
    main = simple_example

    expected_statements = [
        x := py.Constant(1),
        y := py.Mult(x.result, x.result),
        py.Add(y.result, x.result),
    ]
    with pytest.raises(AssertionError):
        assert_structurally_same(main, expected_statements)


def test_wrong_argument_fails_by_default(simple_example):
    main = simple_example

    expected_statements = [
        x := py.Constant(1),
        y := py.Mult(x.result, x.result),
        z := py.Add(y.result, y.result),  # Wrong second argument.
        func.Return(z.result),
    ]
    with pytest.raises(AssertionError):
        assert_structurally_same(main, expected_statements)


def test_wrong_argument_passes_if_check_args_false(simple_example):
    main = simple_example

    expected_statements = [
        x := py.Constant(1),
        y := py.Mult(x.result, x.result),
        z := py.Add(y.result, y.result),  # Wrong second argument.
        func.Return(z.result),
    ]
    assert_structurally_same(main, expected_statements, check_args=False)


def test_wrong_statement_raises_assertion_error(simple_example):
    main = simple_example

    expected_statements = [
        x := py.Constant(1),
        y := py.Mult(x.result, x.result),
        z := py.Mult(y.result, x.result),  # Wrong statement.
        func.Return(z.result),
    ]
    with pytest.raises(AssertionError):
        assert_structurally_same(main, expected_statements)
