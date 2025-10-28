from kirin import ir, rewrite
from kirin.prelude import basic
from kirin.dialects import py, func
from kirin.dialects.func.rewrite import lambdalifting


def test_rewrite_inner_lambda():
    @basic
    def outer():
        def inner(x: int):
            return x + 1

        return inner

    pyconstant_stmt = outer.code.regions[0].blocks[0].stmts.at(0)
    assert isinstance(pyconstant_stmt, py.Constant), "expected a Constant in outer body"
    assert isinstance(
        pyconstant_stmt.value, ir.PyAttr
    ), "expected a PyAttr in outer body"
    assert isinstance(
        pyconstant_stmt.value.data.code, func.Lambda
    ), "expected a lambda Method in outer body"

    rewrite.Walk(lambdalifting.LambdaLifting()).rewrite(outer.code)
    assert isinstance(
        pyconstant_stmt.value.data.code, func.Function
    ), "expected a Function in outer body"


def test_rewrite_inner_lambda_with_captured_vars():
    @basic
    def outer2():
        z = 10
        y = 3 + z

        def inner2(x: int):
            return x + y + 5

        return inner2

    pyconstant_stmt = outer2.code.regions[0].blocks[0].stmts.at(0)
    assert isinstance(pyconstant_stmt, py.Constant), "expected a Constant in outer body"
    assert isinstance(
        pyconstant_stmt.value, ir.PyAttr
    ), "expected a PyAttr in outer body"
    assert isinstance(
        pyconstant_stmt.value.data.code, func.Lambda
    ), "expected a lambda Method in outer body"
    rewrite.Walk(lambdalifting.LambdaLifting()).rewrite(outer2.code)
    assert isinstance(
        pyconstant_stmt.value.data.code, func.Function
    ), "expected a Function in outer body"
