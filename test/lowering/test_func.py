import pytest

from kirin import ir, types
from kirin.dialects import cf, func
from kirin.dialects.py import data, stmts
from kirin.exceptions import DialectLoweringError
from kirin.lowering import Lowering

lowering = Lowering([cf, func, stmts, data])


def test_basic_func():
    def single(n):
        return n + 1

    code = lowering.run(single)
    assert isinstance(code, func.Function)
    assert len(code.body.blocks) == 2
    assert isinstance(code.body.blocks[0].last_stmt, func.Return)
    # NOTE: this is expected behavior, tho this is a dead code
    # this is for matching python behavior, it will be removed after DCE
    assert isinstance(code.body.blocks[1].last_stmt, func.Return)

    def single_2(n):
        return n + 1, n + 2

    code = lowering.run(single_2)
    assert isinstance(code, func.Function)
    assert len(code.body.blocks) == 2
    assert isinstance(code.body.blocks[0].last_stmt, func.Return)
    assert code.body.blocks[0].last_stmt.args[0].type.is_subseteq(types.Tuple)
    assert isinstance(code.body.blocks[1].last_stmt, func.Return)


def test_recursive_func():
    def recursive(n):
        if n == 0:
            return 0
        return recursive(n - 1)

    code = lowering.run(recursive)
    assert isinstance(code, func.Function)
    assert len(code.body.blocks) == 4
    assert isinstance(code.body.blocks[0].last_stmt, cf.ConditionalBranch)
    assert isinstance(code.body.blocks[2].stmts.at(2), func.Call)
    stmt: func.Call = code.body.blocks[2].stmts.at(2)  # type: ignore
    assert isinstance(stmt.callee, ir.BlockArgument)
    assert stmt.callee.type.is_subseteq(func.MethodType)


def test_invalid_func_call():

    def undefined(n):
        return foo(n - 1)  # type: ignore # noqa: F821

    with pytest.raises(DialectLoweringError):
        lowering.run(undefined)

    def calling_python(n):
        return print(n)

    with pytest.raises(
        DialectLoweringError,
        match="`lower_Call_print` is not implemented for builtin function `print`.",
    ):
        lowering.run(calling_python)


def test_func_call():
    def callee(n):
        return n + 1

    def caller(n):
        return callee(n)

    code = lowering.run(callee)
    callee = ir.Method(None, callee, "callee", ["n"], lowering.dialects, code)
    code = lowering.run(caller, globals={"callee": callee})
    assert isinstance(code, func.Function)
    assert len(code.body.blocks) == 2
    stmt = code.body.blocks[0].stmts.at(0)
    assert isinstance(stmt, func.Invoke)
    assert isinstance(stmt.callee, ir.Method)


def test_func_kw_call():
    def callee(n, m):
        return n + m

    def caller(n, m):  # type: ignore
        return callee(n=n, m=m)

    code = lowering.run(callee)
    callee = ir.Method(None, callee, "callee", ["n", "m"], lowering.dialects, code)
    code = lowering.run(caller, globals={"callee": callee})
    assert isinstance(code, func.Function)
    assert len(code.body.blocks) == 2
    stmt = code.body.blocks[0].stmts.at(0)
    assert isinstance(stmt, func.Invoke)
    assert stmt.kwargs == ("n", "m")

    def caller(n, m):
        return callee(n, m=m)

    code = lowering.run(caller, globals={"callee": callee})
    assert isinstance(code, func.Function)
    assert len(code.body.blocks) == 2
    stmt = code.body.blocks[0].stmts.at(0)
    assert isinstance(stmt, func.Invoke)
    assert stmt.kwargs == ("m",)
