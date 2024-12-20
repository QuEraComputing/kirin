import io

from kirin.prelude import basic_no_opt
from kirin.emit.julia import EmitJulia


@basic_no_opt
def foo(x: int, y: int):
    return x + 1, y


@basic_no_opt
def main(x: int, y: int):
    def foo():
        return x

    return foo


io = io.StringIO()
emit = EmitJulia(io, basic_no_opt)
emit.eval(main, ("a", "b"))
