import pytest

from kirin import ir, types
from kirin.prelude import python_no_opt
from kirin.dialects import cf, func
from kirin.lowering import Lowering
from kirin.exceptions import DialectLoweringError

lowering = Lowering(python_no_opt)


def test_basic_func():
    def single(n):
        return n + 1

    code = lowering.run(single)
    assert isinstance(code, func.Function)
    assert len(code.body.blocks) == 1
    assert isinstance(code.body.blocks[0].last_stmt, func.Return)

    def single_2(n):
        return n + 1, n + 2

    code = lowering.run(single_2)
    assert isinstance(code, func.Function)
    assert len(code.body.blocks) == 1
    assert isinstance(code.body.blocks[0].last_stmt, func.Return)
    assert code.body.blocks[0].last_stmt.args[0].type.is_subseteq(types.Tuple)


def test_recursive_func():
    def recursive(n):
        if n == 0:
            return 0
        return recursive(n - 1)

    code = lowering.run(recursive)
    assert isinstance(code, func.Function)
    assert len(code.body.blocks) == 3
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
    assert len(code.body.blocks) == 1
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
    assert len(code.body.blocks) == 1
    stmt = code.body.blocks[0].stmts.at(0)
    assert isinstance(stmt, func.Invoke)
    assert stmt.kwargs == ("n", "m")

    def caller(n, m):
        return callee(n, m=m)

    code = lowering.run(caller, globals={"callee": callee})
    assert isinstance(code, func.Function)
    assert len(code.body.blocks) == 1
    stmt = code.body.blocks[0].stmts.at(0)
    assert isinstance(stmt, func.Invoke)
    assert stmt.kwargs == ("m",)
