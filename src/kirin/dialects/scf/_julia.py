from kirin import ir, emit, interp

from .stmts import For, Yield, IfElse
from ._dialect import dialect


@dialect.register(key="emit.julia")
class JuliaEmit(interp.MethodTable):
    @interp.impl(IfElse)
    def ifelse_(self, emit_: emit.Julia, frame: emit.JuliaFrame, node: IfElse):
        then_block = node.then_body.blocks[0]
        else_block = node.else_body.blocks[0]

        cond = frame.get(node.cond)
        for result in node.results:  # declare the result variables
            frame.write_line(f"local {frame.ssa[result]}")

        frame.write_line(f"if {frame.get(node.cond)}")
        with frame.indent():
            frame.set(then_block.args[0], frame.ssa[then_block.args[0]])
            frame.write_line(f"{frame.ssa[then_block.args[0]]} = {cond}")
            self.walk_yield_block(emit_, frame, node, then_block)

        frame.write_line("else")
        with frame.indent():
            frame.set(else_block.args[0], frame.ssa[else_block.args[0]])
            frame.write_line(f"{frame.ssa[else_block.args[0]]} = {cond}")
            self.walk_yield_block(emit_, frame, node, else_block)
        frame.write_line("end")
        return tuple(frame.ssa[result] for result in node.results)

    @interp.impl(For)
    def for_(self, emit: emit.Julia, frame: emit.JuliaFrame, node: For):
        block = node.body.blocks[0]
        index = block.args[0]
        frame.current_block = block

        for result in node.results:  # declare the result variables
            frame.write_line(f"local {frame.ssa[result]}")

        frame.write_line(f"for {frame.ssa[index]} in {frame.get(node.iterable)}")
        with frame.indent():
            for arg, value in zip(block.args[1:], node.initializers):
                frame.write_line(f"{frame.ssa[arg]} = {frame.get(value)}")
            for arg in block.args:
                frame.set(arg, frame.ssa[arg])
            self.walk_yield_block(emit, frame, node, block)
        frame.write_line("end")

        return tuple(frame.ssa[result] for result in node.results)

    @classmethod
    def walk_yield_block(
        cls,
        emit_: emit.Julia,
        frame: emit.JuliaFrame,
        node: ir.Statement,
        block: ir.Block,
    ):
        for stmt in block.stmts:
            frame.current_stmt = stmt
            if isinstance(stmt, Yield):
                for result, value in zip(node.results, stmt.values):
                    frame.write_line(f"{frame.ssa[result]} = {frame.get(value)}")
                continue

            stmt_results = emit_.frame_eval(frame, stmt)
            if isinstance(stmt_results, tuple):
                frame.set_values(stmt._results, stmt_results)
            elif stmt_results is None:
                continue
            else:
                raise interp.InterpreterError(
                    "unexpected statement result, expected tuple or None"
                )
