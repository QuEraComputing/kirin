from typing import IO, TypeVar

from kirin import emit
from kirin.interp import MethodTable, impl
from kirin.emit.julia import EmitJulia

from .stmts import Call, Invoke, Return, Function, ConstantNone
from .dialect import dialect

IO_t = TypeVar("IO_t", bound=IO)


@dialect.register(key="emit.julia")
class JuliaMethodTable(MethodTable):

    @impl(Function)
    def emit_function(
        self, interp: EmitJulia[IO_t], frame: emit.EmitFrame, stmt: Function
    ):
        args = tuple(interp.ssa_id[x] for x in stmt.body.blocks[0].args)
        interp.write(f"function {stmt.sym_name}({', '.join(args)})")
        frame.indent += 1
        interp.run_ssacfg_region(stmt.body, args)
        frame.indent -= 1
        interp.newline(frame)
        interp.write("end")
        return ()

    @impl(Return)
    def emit_return(self, interp: EmitJulia[IO_t], frame: emit.EmitFrame, stmt: Return):
        interp.newline(frame)
        interp.write(f"return {frame.get(stmt.value)}")
        return ()

    @impl(ConstantNone)
    def emit_constant_none(
        self, interp: EmitJulia[IO_t], frame: emit.EmitFrame, stmt: ConstantNone
    ):
        return ("nothing",)

    @impl(Call)
    def emit_call(self, interp: EmitJulia[IO_t], frame: emit.EmitFrame, stmt: Call):
        return (
            f"{frame.get(stmt.callee)}({', '.join(frame.get_values(stmt.inputs))})",
        )

    @impl(Invoke)
    def emit_invoke(self, interp: EmitJulia[IO_t], frame: emit.EmitFrame, stmt: Invoke):
        return (f"{stmt.callee.sym_name}({', '.join(frame.get_values(stmt.inputs))})",)
