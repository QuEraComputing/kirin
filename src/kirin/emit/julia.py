from __future__ import annotations

from typing import IO, Generic, TypeVar
from dataclasses import dataclass

from kirin import ir, interp

from .abc import EmitABC, EmitFrame

IO_t = TypeVar("IO_t", bound=IO)


@dataclass
class JuliaFrame(EmitFrame[str], Generic[IO_t]):
    io: IO_t


@dataclass
class EmitJulia(EmitABC[JuliaFrame, str], Generic[IO_t]):
    """Julia code generator for the IR.

    This class generates Julia code from the IR.
    It is used to generate Julia code for the IR.
    """

    keys = ("julia",)
    void = ""
    io: IO_t

    def initialize_frame(
        self, node: ir.Statement, *, has_parent_access: bool = False
    ) -> JuliaFrame:
        return JuliaFrame(node, self.io, has_parent_access=has_parent_access)

    def run(self, node: ir.Method | ir.Statement):
        if isinstance(node, ir.Method):
            node = node.code

        with self.eval_context():
            self.eval(node)
        self.io.flush()

    def emit_attribute(self, frame: JuliaFrame, node: ir.Statement) -> str:
        method = self.registry.get(interp.Signature(type(node)))
        if method is None:
            raise ValueError(f"Method not found for node: {node}")
        return method(self, frame, node)
