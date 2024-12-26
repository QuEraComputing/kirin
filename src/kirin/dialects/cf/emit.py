from typing import IO, TypeVar

from kirin import emit
from kirin.interp import MethodTable, impl
from kirin.emit.julia import EmitJulia

from .stmts import Assert, Branch, ConditionalBranch
from .dialect import dialect

IO_t = TypeVar("IO_t", bound=IO)


@dialect.register(key="emit.julia")
class JuliaMethodTable(MethodTable):

    @impl(Assert)
    def emit_assert(
        self, interp: EmitJulia[IO_t], frame: emit.EmitStrFrame, stmt: Assert
    ):
        interp.writeln(
            frame, f"@assert {frame.get(stmt.condition)} {frame.get(stmt.message)}"
        )
        return ()

    @impl(Branch)
    def emit_branch(
        self, interp: EmitJulia[IO_t], frame: emit.EmitStrFrame, stmt: Branch
    ):
        interp.writeln(frame, f"@goto {interp.block_id[stmt.successor]};")
        return ()

    @impl(ConditionalBranch)
    def emit_cbr(
        self, interp: EmitJulia[IO_t], frame: emit.EmitStrFrame, stmt: ConditionalBranch
    ):
        cond = frame.get(stmt.cond)
        interp.writeln(frame, f"if {cond}")
        frame.indent += 1
        ori = frame.get_values(stmt.then_successor.args)
        values = frame.get_values(stmt.then_arguments)
        for x, y in zip(ori, values):
            interp.writeln(frame, f"{x} = {y};")
        interp.writeln(frame, f"@goto {interp.block_id[stmt.then_successor]};")
        frame.indent -= 1
        interp.writeln(frame, "else")
        frame.indent += 1
        ori = frame.get_values(stmt.else_successor.args)
        values = frame.get_values(stmt.else_arguments)
        for x, y in zip(ori, values):
            interp.writeln(frame, f"{x} = {y};")
        interp.writeln(frame, f"@goto {interp.block_id[stmt.else_successor]};")
        frame.indent -= 1
        interp.writeln(frame, "end")
        return ()
