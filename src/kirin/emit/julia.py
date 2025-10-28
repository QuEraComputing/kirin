from __future__ import annotations

from typing import IO, Generic, TypeVar
from contextlib import contextmanager
from dataclasses import field, dataclass

from kirin import ir, interp
from kirin.idtable import IdTable

from .abc import EmitABC, EmitFrame

IO_t = TypeVar("IO_t", bound=IO)


@dataclass
class JuliaFrame(EmitFrame[str], Generic[IO_t]):
    io: IO_t
    ssa: IdTable[ir.SSAValue] = field(
        default_factory=lambda: IdTable[ir.SSAValue](prefix="ssa_")
    )
    block: IdTable[ir.Block] = field(
        default_factory=lambda: IdTable[ir.Block](prefix="block_")
    )
    _indent: int = 0

    def write(self, value):
        self.io.write(value)

    def write_line(self, value):
        self.write("    " * self._indent + value + "\n")

    @contextmanager
    def indent(self):
        self._indent += 1
        yield
        self._indent -= 1


@dataclass
class Julia(EmitABC[JuliaFrame, str], Generic[IO_t]):
    """Julia code generator for the IR.

    This class generates Julia code from the IR.
    It is used to generate Julia code for the IR.
    """

    keys = ("emit.julia",)
    void = ""

    # some states
    io: IO_t

    def initialize(self):
        super().initialize()
        return self

    def initialize_frame(
        self, node: ir.Statement, *, has_parent_access: bool = False
    ) -> JuliaFrame:
        return JuliaFrame(node, self.io, has_parent_access=has_parent_access)

    def run(self, node: ir.Method | ir.Statement):
        if isinstance(node, ir.Method):
            node = node.code

        with self.eval_context():
            self.callables.add(node)
            self.callable_to_emit.append(node)
            while self.callable_to_emit:
                callable = self.callable_to_emit.pop()
                if callable is None:
                    break
                self.eval(callable)
                self.io.flush()
        return

    def frame_call(
        self, frame: JuliaFrame, node: ir.Statement, *args: str, **kwargs: str
    ) -> str:
        return f"{args[0]}({', '.join(args[1:])})"

    def get_attribute(self, frame: JuliaFrame, node: ir.Attribute) -> str:
        method = self.registry.get(interp.Signature(type(node)))
        if method is None:
            raise ValueError(f"Method not found for node: {node}")
        return method(self, frame, node)
