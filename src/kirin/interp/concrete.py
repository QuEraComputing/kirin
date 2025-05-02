from __future__ import annotations

from typing import Any
from dataclasses import dataclass

from kirin import ir

from .abc import InterpreterABC
from .frame import Frame


@dataclass
class Interpreter(InterpreterABC[Frame[Any], Any]):
    keys = ("main",)
    void = None

    def initialize_frame(
        self, node: ir.Statement, *, has_parent_access: bool = False
    ) -> Frame[Any]:
        """Initialize the frame for the given node."""
        return Frame(node, has_parent_access=has_parent_access)

    def run(self, method: ir.Method, *args, **kwargs):
        return self.call(method, method, *args, **kwargs)
