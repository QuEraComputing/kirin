from __future__ import annotations

from typing import IO, TypeVar

from kirin import emit, interp

from .stmts import Invoke, Return, Function
from ._dialect import dialect


@dialect.register(key="emit.julia")
class Julia(interp.MethodTable):

    IO_t = TypeVar("IO_t", bound=IO)

    @interp.impl(Return)
    def return_(
        self, emit: emit.Julia[IO_t], frame: emit.JuliaFrame[IO_t], node: Return
    ):
        value = frame.get(node.value)
        frame.write_line(f"return {value}")

    @interp.impl(Invoke)
    def invoke(
        self, emit: emit.Julia[IO_t], frame: emit.JuliaFrame[IO_t], node: Invoke
    ):
        func_name = emit.callables.get(node.callee.code)
        if func_name is None:
            emit.callable_to_emit.append(node.callee.code)
            func_name = emit.callables.add(node.callee.code)

        _, call_expr = emit.call(
            node.callee.code, func_name, *frame.get_values(node.args)
        )
        frame.write_line(f"{frame.ssa[node.result]} = {call_expr}")
        return (frame.ssa[node.result],)

    @interp.impl(Function)
    def function(
        self, emit_: emit.Julia[IO_t], frame: emit.JuliaFrame[IO_t], node: Function
    ):
        func_name = emit_.callables[node]
        frame.set(node.body.blocks[0].args[0], func_name)
        argnames_: list[str] = []
        for arg in node.body.blocks[0].args[1:]:
            frame.set(arg, name := frame.ssa[arg])
            argnames_.append(name)

        argnames = ", ".join(argnames_)
        frame.write_line(f"function {func_name}({argnames})")
        with frame.indent():
            for block in node.body.blocks:
                frame.current_block = block
                frame.write_line(f"@label {frame.block[block]}")
                for arg in block.args:
                    frame.set(arg, frame.ssa[arg])

                for stmt in block.stmts:
                    frame.current_stmt = stmt
                    stmt_results = emit_.frame_eval(frame, stmt)

                    match stmt_results:
                        case tuple():
                            frame.set_values(stmt._results, stmt_results)
                        case _:
                            continue
        frame.write_line("end\n")
