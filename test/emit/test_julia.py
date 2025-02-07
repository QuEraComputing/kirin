import io

from kirin import ir
from kirin.prelude import basic_no_opt
from kirin.emit.julia import EmitJulia


def emit(fn: ir.Method):
    with io.StringIO() as file:
        emit_ = EmitJulia(basic_no_opt, file)
        emit_.run(fn, tuple(fn.arg_names[1:])).expect()
        return file.getvalue()


def test_func():
    @basic_no_opt
    def emit_func(x: int, y: int):
        def foo():
            return x

        return foo

    generated = emit(emit_func)
    print(generated)
    assert "function emit_func(x::Int, y::Int)" in generated
    assert "@label block_0;" in generated
    assert "function foo()" in generated
    assert "@label block_1;" in generated
    assert "return x" in generated
    assert "return foo" in generated


def test_py_stmts():
    @basic_no_opt
    def emit_py_stmts(x: int, y: int):
        a1 = x * (x + y - x + 1)
        a2 = x == y
        a3 = x >= y
        a4 = x <= y
        a5 = x != y
        a6 = x > y
        a7 = x < y
        return a1, a2, a3, a4, a5, a6, a7

    generated = emit(emit_py_stmts)
    assert "var_0 = x + y" in generated
    assert "var_1 = var_0 - x" in generated
    assert "var_2 = var_1 + 1" in generated
    assert "a1 = x * var_2" in generated
    assert "a2 = x == y" in generated
    assert "a3 = x >= y" in generated
    assert "a4 = x <= y" in generated
    assert "a5 = x != y" in generated
    assert "a6 = x > y" in generated
    assert "a7 = x < y" in generated
    assert "var_3 = (a1, a2, a3, a4, a5, a6, a7)" in generated


def test_cf():
    @basic_no_opt
    def emit_cf(x: int, y: int):
        if x > y:
            return x
        else:
            return y

    emit_cf.print()
    generated = emit(emit_cf)

    assert "var_0 = x > y" in generated
    assert "if var_0" in generated
    assert "@goto block_1" in generated
    assert "@goto block_2" in generated
