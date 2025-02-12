from abc import ABC
from typing import IO, Generic, TypeVar
from dataclasses import field, dataclass

from kirin import ir, interp, idtable
from kirin.emit.abc import EmitABC, EmitFrame

IO_t = TypeVar("IO_t", bound=IO)


@dataclass
class EmitStrFrame(EmitFrame[str]):
    indent: int = 0
    captured: dict[ir.SSAValue, tuple[str, ...]] = field(default_factory=dict)


@dataclass
class EmitStr(EmitABC[EmitStrFrame, str], ABC, Generic[IO_t]):
    void = ""
    file: IO_t
    prefix: str = field(default="", kw_only=True)
    prefix_if_none: str = field(default="var_", kw_only=True)

    def initialize(self):
        super().initialize()
        self.ssa_id = idtable.IdTable[ir.SSAValue](
            prefix=self.prefix, prefix_if_none=self.prefix_if_none
        )
        self.block_id = idtable.IdTable[ir.Block](prefix=self.prefix + "block_")
        return self

    def new_frame(self, code: ir.Statement) -> EmitStrFrame:
        return EmitStrFrame.from_func_like(code)

    def run_method(
        self, method: ir.Method, args: tuple[str, ...]
    ) -> tuple[EmitStrFrame, str]:
        if len(self.state.frames) >= self.max_depth:
            raise interp.InterpreterError("maximum recursion depth exceeded")
        return self.run_callable(method.code, (method.sym_name,) + args)

    def write(self, *args):
        for arg in args:
            self.file.write(arg)

    def newline(self, frame: EmitStrFrame):
        self.file.write("\n" + "  " * frame.indent)

    def writeln(self, frame: EmitStrFrame, *args):
        self.newline(frame)
        self.write(*args)
