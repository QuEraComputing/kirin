from abc import ABC
from typing import IO, Generic, TypeVar
from contextlib import contextmanager
from dataclasses import field, dataclass

from kirin import ir, interp, idtable
from kirin.emit.abc import EmitABC, EmitFrame

from .exceptions import EmitError

IO_t = TypeVar("IO_t", bound=IO)


@dataclass
class EmitStrFrame(EmitFrame[str]):
    indent: int = 0
    ssa_id: idtable.IdTable[ir.SSAValue] = field(
        default_factory=lambda: idtable.IdTable(prefix="", prefix_if_none="var_")
    )
    captured: dict[ir.SSAValue, tuple[str, ...]] = field(default_factory=dict)

    @contextmanager
    def set_indent(self, indent: int):
        self.indent += indent
        try:
            yield
        finally:
            self.indent -= indent


@dataclass
class EmitStr(EmitABC[EmitStrFrame, str], ABC, Generic[IO_t]):
    void = ""
    prefix: str = field(default="", kw_only=True)
    prefix_if_none: str = field(default="var_", kw_only=True)

    def initialize(self):
        super().initialize()
        self.ssa_id = idtable.IdTable[ir.SSAValue](
            prefix=self.prefix, prefix_if_none=self.prefix_if_none
        )
        self.block_id = idtable.IdTable[ir.Block](prefix=self.prefix + "block_")
        return self

    def initialize_frame(
        self, code: ir.Statement, *, has_parent_access: bool = False
    ) -> EmitStrFrame:
        return EmitStrFrame(code, has_parent_access=has_parent_access)

    def run_method(
        self, method: ir.Method, args: tuple[str, ...]
    ) -> tuple[EmitStrFrame, str]:
        if self.state.depth >= self.max_depth:
            raise interp.InterpreterError("maximum recursion depth exceeded")
        return self.run_callable(method.code, (method.sym_name,) + args)

    def run_callable_region(
        self,
        frame: EmitStrFrame,
        code: ir.Statement,
        region: ir.Region,
        args: tuple[str, ...],
    ) -> str:
        lines = []
        for block in region.blocks:
            block_id = self.block_id[block]
            frame.block_ref[block] = block_id
            with frame.set_indent(1):
                self.run_succ(
                    frame, interp.Successor(block, frame.get_values(block.args))
                )
                self.write(f"@label {block_id};")

                for each_stmt in block.stmts:
                    results = self.eval_stmt(frame, each_stmt)
                    if isinstance(results, tuple):
                        frame.set_values(each_stmt.results, results)
                    elif results is not None:
                        raise EmitError(
                            f"Unexpected result {results} from statement {each_stmt.name}"
                        )

    def write(self, *args):
        for arg in args:
            self.file.write(arg)

    def newline(self, frame: EmitStrFrame):
        self.file.write("\n" + "  " * frame.indent)

    def writeln(self, frame: EmitStrFrame, *args):
        self.newline(frame)
        self.write(*args)
