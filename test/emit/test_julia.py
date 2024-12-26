import io

from kirin import ir
from kirin.prelude import basic_no_opt
from kirin.emit.julia import EmitJulia


def emit(fn: ir.Method):
    with io.StringIO() as file:
        emit_ = EmitJulia(file, basic_no_opt)
        emit_.eval(fn, tuple(fn.arg_names[1:]))
        return file.getvalue()


def test_func():
    @basic_no_opt
    def emit_func(x: int, y: int):
        def foo():
            return x

        return foo

    generated = emit(emit_func)
    assert "function emit_func(x::Int, y::Int)" in generated
    assert "@label block_0;" in generated
    assert "function foo()" in generated
    assert "@label block_1;" in generated
    assert "return x" in generated
    assert "@label block_2;" in generated
    assert "return nothing" in generated
    assert "return foo" in generated
    assert "return nothing" in generated
