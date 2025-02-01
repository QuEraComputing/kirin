from functools import partial

import pytest

from kirin.prelude import python_no_opt
from kirin.dialects import cf, func
from kirin.lowering import Lowering
from kirin.exceptions import DialectLoweringError

lowering = Lowering(python_no_opt)


def test_closure():
    def closure(x):
        def inner(y):
            return x + y

        return inner

    code = lowering.run(closure)
    assert isinstance(code, func.Function)

    def will_error(x):
        @partial
        def inner(y):
            return x + y

        return inner

    with pytest.raises(DialectLoweringError):
        lowering.run(will_error)


def test_closure_branch():
    def closure_branch(m: int, n: int):

        def inner(x: int, y: int):
            if x > y:
                return m, n

        return inner

    code = lowering.run(closure_branch)
    code.print()
    assert isinstance(code, func.Function)
    first_block = code.body.blocks[0]
    lambda_stmt = first_block.first_stmt
    assert isinstance(lambda_stmt, func.Lambda)
    assert lambda_stmt.args[0] is first_block.args[1]
    first_lambda_block = lambda_stmt.body.blocks[0]
    assert len(first_lambda_block.args) == 3
    get_field_n = first_lambda_block.stmts.at(0)
    assert isinstance(get_field_n, func.GetField)
    assert get_field_n.field == 1
    get_field_m = first_lambda_block.stmts.at(1)
    assert isinstance(get_field_m, func.GetField)
    assert get_field_m.field == 0
    assert isinstance(first_lambda_block.last_stmt, cf.ConditionalBranch)
    assert first_lambda_block.last_stmt.then_successor is lambda_stmt.body.blocks[1]
    assert first_lambda_block.last_stmt.else_successor is lambda_stmt.body.blocks[2]
    assert isinstance(lambda_stmt.body.blocks[1].last_stmt, func.Return)
