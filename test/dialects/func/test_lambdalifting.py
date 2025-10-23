from kirin import ir, rewrite
from kirin.prelude import basic
from kirin.dialects import py, func


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

    rewrite.Walk(func.lambdalifting.LambdaLifting()).rewrite(outer.code)
    assert isinstance(
        pyconstant_stmt.value.data.code, func.Function
    ), "expected a Function in outer body"


test_rewrite_inner_lambda()
