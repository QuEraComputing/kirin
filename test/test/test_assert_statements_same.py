import pytest

from kirin.test import assert_statements_same
from kirin.dialects import py


def test_same_statements_pass():
    statement_1 = py.Constant(1)
    statement_2 = py.Constant(1)
    assert_statements_same(statement_1, statement_2)


def test_different_statements_fail():
    statement_1 = py.Constant(1)
    statement_2 = py.Mult(statement_1.result, statement_1.result)
    with pytest.raises(AssertionError):
        assert_statements_same(statement_1, statement_2)


def test_same_statement_different_args_fails_1():
    statement_1 = py.Constant(1)
    statement_2 = py.Constant(2)
    with pytest.raises(AssertionError):
        assert_statements_same(statement_1, statement_2)


def test_same_statement_different_args_fails_2():
    arg_1 = py.Constant(1)
    arg_2 = py.Constant(2)
    statement_1 = py.Mult(arg_1.result, arg_1.result)
    statement_2 = py.Mult(arg_2.result, arg_2.result)
    with pytest.raises(AssertionError):
        assert_statements_same(statement_1, statement_2)


def test_same_statement_different_args_check_args_false_passes():
    arg_1 = py.Constant(1)
    arg_2 = py.Constant(2)
    statement_1 = py.Mult(arg_1.result, arg_1.result)
    statement_2 = py.Mult(arg_2.result, arg_2.result)
    assert_statements_same(statement_1, statement_2, check_args=False)
