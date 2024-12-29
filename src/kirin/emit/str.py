from typing import IO, Generic, TypeVar, Iterable
from dataclasses import field, dataclass

from kirin import ir, interp, idtable
from kirin.emit.abc import EmitABC, EmitFrame
from kirin.exceptions import InterpreterError

IO_t = TypeVar("IO_t", bound=IO)


@dataclass
class EmitStrFrame(EmitFrame[str]):
    indent: int = 0
    captured: dict[ir.SSAValue, tuple[str, ...]] = field(default_factory=dict)


class EmitStr(EmitABC[EmitStrFrame, str], Generic[IO_t]):

    def __init__(
        self,
        file: IO_t,
        dialects: ir.DialectGroup | Iterable[ir.Dialect],
        *,
        fuel: int | None = None,
        max_depth: int = 128,
        max_python_recursion_depth: int = 8192,
        prefix: str = "",
        prefix_if_none: str = "var_",
    ):
        super().__init__(
            dialects,
            bottom="",
            fuel=fuel,
            max_depth=max_depth,
            max_python_recursion_depth=max_python_recursion_depth,
        )
        self.file = file
        self.ssa_id = idtable.IdTable[ir.SSAValue](
            prefix=prefix, prefix_if_none=prefix_if_none
        )
        self.block_id = idtable.IdTable[ir.Block](prefix=prefix + "block_")

    def new_frame(self, code: ir.Statement) -> EmitStrFrame:
        return EmitStrFrame.from_func_like(code)

    def run_method(
        self, method: ir.Method, args: tuple[str, ...]
    ) -> str | interp.Err[str]:
        if len(self.state.frames) >= self.max_depth:
            raise InterpreterError("maximum recursion depth exceeded")
        return self.run_callable(method.code, (method.sym_name,) + args)

    def write(self, *args):
        for arg in args:
            self.file.write(arg)

    def newline(self, frame: EmitStrFrame):
        self.file.write("\n" + "  " * frame.indent)

    def writeln(self, frame: EmitStrFrame, *args):
        self.newline(frame)
        self.write(*args)
