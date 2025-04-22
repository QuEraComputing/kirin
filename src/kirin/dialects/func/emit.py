from typing import IO, TypeVar

from kirin import emit
from kirin.interp import MethodTable, InterpreterError, impl
from kirin.emit.julia import EmitJulia

from .stmts import Call, Invoke, Lambda, Return, Function, GetField, ConstantNone
from .dialect import dialect

IO_t = TypeVar("IO_t", bound=IO)


@dialect.register(key="emit.julia")
class JuliaMethodTable(MethodTable):

    @impl(Function)
    def emit_function(
        self, emit: EmitJulia[IO_t], frame: emit.EmitStrFrame, stmt: Function
    ):
        fn_args = stmt.body.blocks[0].args[1:]
        argnames = tuple(emit.ssa_id[arg] for arg in fn_args)
        argtypes = tuple(emit.emit_attribute(x.type) for x in fn_args)
        args = [f"{name}::{type}" for name, type in zip(argnames, argtypes)]
        emit.write(f"function {stmt.sym_name}({', '.join(args)})")
        with frame.set_indent(1):
            for block in stmt.body.blocks:
                block_id = emit.block_id[block]
                frame.block_ref[block] = block_id
                emit.newline(frame)
                emit.write(f"@label {block_id};")

                for each_stmt in block.stmts:
                    results = emit.eval_stmt(frame, each_stmt)
                    if isinstance(results, tuple):
                        frame.set_values(each_stmt.results, results)
                    elif results is not None:
                        raise InterpreterError(
                            f"Unexpected result {results} from statement {each_stmt.name}"
                        )
        emit.writeln(frame, "end")
        return ()

    @impl(Return)
    def emit_return(
        self, interp: EmitJulia[IO_t], frame: emit.EmitStrFrame, stmt: Return
    ):
        interp.writeln(frame, f"return {frame.get(stmt.value)}")
        return ()

    @impl(ConstantNone)
    def emit_constant_none(
        self, interp: EmitJulia[IO_t], frame: emit.EmitStrFrame, stmt: ConstantNone
    ):
        return ("nothing",)

    @impl(Call)
    def emit_call(self, interp: EmitJulia[IO_t], frame: emit.EmitStrFrame, stmt: Call):
        if stmt.kwargs:
            raise InterpreterError("cannot emit kwargs for dyanmic calls")
        return (
            f"{frame.get(stmt.callee)}({', '.join(frame.get_values(stmt.inputs))})",
        )

    @impl(Invoke)
    def emit_invoke(
        self, interp: EmitJulia[IO_t], frame: emit.EmitStrFrame, stmt: Invoke
    ):
        args = interp.permute_values(
            stmt.callee.arg_names, frame.get_values(stmt.inputs), stmt.kwargs
        )
        return (f"{stmt.callee.sym_name}({', '.join(args)})",)

    @impl(Lambda)
    def emit_lambda(
        self, interp: EmitJulia[IO_t], frame: emit.EmitStrFrame, stmt: Lambda
    ):
        args = tuple(interp.ssa_id[x] for x in stmt.body.blocks[0].args[1:])
        frame.set_values(stmt.body.blocks[0].args, (stmt.sym_name,) + args)
        frame.captured[stmt.body.blocks[0].args[0]] = frame.get_values(stmt.captured)
        interp.writeln(frame, f"function {stmt.sym_name}({', '.join(args[1:])})")
        frame.set_indent += 1
        interp.run_ssacfg_region(frame, stmt.body, args)
        frame.set_indent -= 1
        interp.writeln(frame, "end")
        return (stmt.sym_name,)

    @impl(GetField)
    def emit_getfield(
        self, interp: EmitJulia[IO_t], frame: emit.EmitStrFrame, stmt: GetField
    ):
        return (frame.captured[stmt.obj][stmt.field],)
