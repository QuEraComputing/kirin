"""Tools for testing methods."""

from typing import List

from kirin.ir import Method, Statement
from kirin.dialects import func


def assert_statements_same(
    statement_1: Statement, statement_2: Statement, check_args: bool = True
):
    """Assert statements are the same and recursively check arguments.

    Args:
        statement_1 (Statement): First statement to compare.
        statement_2 (Statement): Second statement to compare.
        check_args (bool): Recursively check arguments if True.
    """
    assert type(statement_1) is type(
        statement_2
    ), "Statements have different type: {} vs {}".format(
        type(statement_1).__name__, type(statement_2).__name__
    )
    assert statement_1.attributes == statement_2.attributes, (
        "Statements have different attributes" ""
    )
    assert len(statement_1.args.field) == len(
        statement_2.args.field
    ), "Arguments have different lengths"
    if check_args:
        for arg_1, arg_2 in zip(statement_1.args.field, statement_2.args.field):
            assert isinstance(arg_1.owner, Statement)
            assert isinstance(arg_2.owner, Statement)
            assert_statements_same(arg_1.owner, arg_2.owner, check_args=True)


def assert_structurally_same(
    method: Method,
    expected_stmts: List[Statement],
    check_args: bool = True,
    debug: bool = False,
):
    """Assert Method is structurally the same as list of Statements.

    Args:
        method (Method): A kirin Method.
        expected_stmts: A list of statements to compare with.
        check_args (bool): Recursively check arguments of statements if True.
        debug (bool): Pretty-print statements in method until fail.
    """

    assert isinstance(method.code, func.Function)
    new_stmts = list(method.code.body.blocks[0].stmts)
    assert len(new_stmts) == len(expected_stmts), "Methods different lengths"
    for new_stmt, expected_stmt in zip(new_stmts, expected_stmts):
        if debug:
            new_stmt.print()
        assert_statements_same(new_stmt, expected_stmt, check_args=check_args)
