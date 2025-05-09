import ast

import rich

from kirin import ir, decl, emit, types, interp, lowering

dialect = ir.Dialect("debug")


class InfoLowering(lowering.FromPythonCall):

    def lower(
        self, stmt: type, state: lowering.State, node: ast.Call
    ) -> lowering.Result:
        if len(node.args) == 0:
            raise lowering.BuildError("info() requires at least one argument")

        msg = state.lower(node.args[0]).expect_one()
        if len(node.args) > 1:
            inputs = tuple(state.lower(arg).expect_one() for arg in node.args[1:])
        else:
            inputs = ()
        state.current_frame.push(Info(msg=msg, inputs=inputs))


@decl.statement(dialect=dialect)
class Info(ir.Statement):
    """print debug information.

    This statement is used to print debug information during
    execution. The compiler has freedom to choose how to print
    the information and send it back to the caller. Note that
    in the case of heterogeneous hardware, this may not be printed
    on the same device as the caller but instead being a log.
    """

    traits = frozenset({InfoLowering()})
    msg: ir.SSAValue = decl.info.argument(types.String)
    inputs: tuple[ir.SSAValue, ...] = decl.info.argument()


@lowering.wraps(Info)
def info(msg: str, *inputs) -> None: ...


@dialect.register(key="main")
class ConcreteMethods(interp.MethodTable):

    @interp.impl(Info)
    def info(self, interp: interp.Interpreter, frame: interp.Frame, stmt: Info):
        # print("INFO:", frame.get(stmt.msg))
        rich.print(
            "[dim]┌───────────────────────────────────────────────────────────────[/dim]"
        )
        rich.print("[dim]│[/dim] [bold cyan]INFO:[/bold cyan] ", end="", sep="")
        print(frame.get(stmt.msg))
        for input in stmt.inputs:
            rich.print(
                "[dim]│[/dim] ",
                input.name or "unknown",
                "[dim] = [/dim]",
                end="",
                sep="",
            )
            print(frame.get(input))
        rich.print(
            "[dim]└───────────────────────────────────────────────────────────────[/dim]"
        )


@dialect.register(key="emit.julia")
class JuliaEmit(interp.MethodTable):
    @interp.impl(Info)
    def info(self, emit: emit.Julia, frame: emit.JuliaFrame, stmt: Info):
        msg = frame.get(stmt.msg)
        inputs = " ".join(frame.get(input) for input in stmt.inputs)
        frame.write_line(f'@info "{msg[1:-1]}" {inputs}'.strip())
