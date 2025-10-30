from typing import cast

from kirin import rewrite
from kirin.prelude import basic
from kirin.dialects import py, func
from kirin.dialects.func.rewrite import closurefield


def test_rewrite_closure_inner_lambda():
    @basic
    def outer(y: int):
        def inner(x: int):
            return x * y + 1

        return inner

    inner_ker = outer(y=10)

    @basic
    def main_lambda(z: int):
        return inner_ker(z)

    main_invoke = main_lambda.code.regions[0].blocks[0].stmts.at(0)
    inner_lambda = cast(func.Invoke, main_invoke).callee.code
    inner_getfield_stmt = inner_lambda.regions[0].blocks[0].stmts.at(0)
    assert isinstance(
        inner_getfield_stmt, func.GetField
    ), "expected GetField before rewrite"

    rewrite.Walk(closurefield.ClosureField()).rewrite(main_lambda.code)

    inner_getfield_stmt = inner_lambda.regions[0].blocks[0].stmts.at(0)
    assert isinstance(
        inner_getfield_stmt, py.Constant
    ), "GetField should be lowered to Constant"


def test_rewrite_closure_no_fields():
    @basic
    def bar():
        def goo(x: int):
            a = (3, 4)
            return a[0]

        def boo(y):
            return goo(y) + 1

        return boo(4)

    before = bar.code.regions[0].blocks[0].stmts.at(0)
    rewrite.Walk(closurefield.ClosureField()).rewrite(bar.code)
    after = bar.code.regions[0].blocks[0].stmts.at(0)
    assert before is after
