from typing import IO, TypeVar

from kirin import emit
from kirin.interp import Successor, MethodTable, impl
from kirin.emit.julia import EmitJulia

from .stmts import Branch, ConditionalBranch
from .dialect import dialect

IO_t = TypeVar("IO_t", bound=IO)


@dialect.register(key="emit.julia")
class JuliaMethodTable(MethodTable):

    @impl(Branch)
    def emit_branch(
        self, interp: EmitJulia[IO_t], frame: emit.EmitStrFrame, stmt: Branch
    ):
        interp.writeln(frame, f"@goto {interp.block_id[stmt.successor]};")
        frame.worklist.append(
            Successor(stmt.successor, frame.get_values(stmt.arguments))
        )
        return ()

    @impl(ConditionalBranch)
    def emit_cbr(
        self, interp: EmitJulia[IO_t], frame: emit.EmitStrFrame, stmt: ConditionalBranch
    ):
        cond = frame.get(stmt.cond)
        interp.writeln(frame, f"if {cond}")
        frame.indent += 1
        values = frame.get_values(stmt.then_arguments)
        block_values = tuple(interp.ssa_id[x] for x in stmt.then_successor.args)
        frame.set_values(stmt.then_successor.args, block_values)
        for x, y in zip(block_values, values):
            interp.writeln(frame, f"{x} = {y};")
        interp.writeln(frame, f"@goto {interp.block_id[stmt.then_successor]};")
        frame.indent -= 1
        interp.writeln(frame, "else")
        frame.indent += 1

        values = frame.get_values(stmt.else_arguments)
        block_values = tuple(interp.ssa_id[x] for x in stmt.else_successor.args)
        frame.set_values(stmt.else_successor.args, block_values)
        for x, y in zip(block_values, values):
            interp.writeln(frame, f"{x} = {y};")
        interp.writeln(frame, f"@goto {interp.block_id[stmt.else_successor]};")
        frame.indent -= 1
        interp.writeln(frame, "end")

        frame.worklist.append(
            Successor(stmt.then_successor, frame.get_values(stmt.then_arguments))
        )
        frame.worklist.append(
            Successor(stmt.else_successor, frame.get_values(stmt.else_arguments))
        )
        return ()
