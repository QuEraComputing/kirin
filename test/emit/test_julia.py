import io

from kirin.prelude import basic_no_opt
from kirin.emit.julia import EmitJulia


@basic_no_opt
def foo(x: int, y: int):
    return x + 1, y


@basic_no_opt
def main(x: int, y: int):
    assert x == y, "x != y"

    def foo():
        return x

    if True:
        return foo()
    else:
        return foo


with io.StringIO() as f:
    emit = EmitJulia(f, basic_no_opt)
    emit.eval(main, ("a", "b"))
    generated = f.getvalue()
    print(generated)
    assert "function main(a::Int, b::Int)" in generated
