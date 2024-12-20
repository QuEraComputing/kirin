from typing import IO, Generic, TypeVar, Iterable

from kirin import ir, interp, idtable
from kirin.emit.abc import EmitABC, EmitFrame
from kirin.exceptions import InterpreterError

IO_t = TypeVar("IO_t", bound=IO)


class EmitStr(EmitABC[EmitFrame[str], str], Generic[IO_t]):
    empty_result = ""

    def __init__(
        self,
        file: IO_t,
        dialects: ir.DialectGroup | Iterable[ir.Dialect],
        *,
        fuel: int | None = None,
        max_depth: int = 128,
        max_python_recursion_depth: int = 8192,
        prefix: str = "",
    ):
        super().__init__(
            dialects,
            fuel=fuel,
            max_depth=max_depth,
            max_python_recursion_depth=max_python_recursion_depth,
        )
        self.file = file
        self.ssa_id = idtable.IdTable[ir.SSAValue](prefix=prefix + "var_")
        self.block_id = idtable.IdTable[ir.Block](prefix=prefix + "block_")

    def new_frame(self, code: ir.Statement) -> EmitFrame[str]:
        return EmitFrame.from_func_like(code)

    def run_method(
        self, method: ir.Method, args: tuple[str, ...]
    ) -> str | interp.Err[str]:
        if len(self.state.frames) >= self.max_depth:
            raise InterpreterError("maximum recursion depth exceeded")
        return self.run_callable(method.code, (method.sym_name,) + args)

    def write(self, *args):
        for arg in args:
            self.file.write(arg)

    def newline(self, frame: EmitFrame[str]):
        self.file.write("\n" + "  " * frame.indent)
