# NOTE: this module is only interface, will be used inside
# the `ir` module try to minimize the dependencies as much
# as possible

from __future__ import annotations

import ast
from abc import ABC
from typing import TYPE_CHECKING

from kirin.lowering.exception import BuildError

if TYPE_CHECKING:
    from kirin import ir
    from kirin.lowering.state import State


class FromPythonAST(ABC):

    @property
    def names(self) -> list[str]:  # show the name without lower_
        return [name[6:] for name in dir(self) if name.startswith("lower_")]

    def lower(self, state: State[ast.AST], node: ast.AST) -> ir.SSAValue | None:
        """Entry point of dialect specific lowering."""
        return getattr(self, f"lower_{node.__class__.__name__}", self.unreachable)(
            state, node
        )

    def unreachable(self, state: State[ast.AST], node: ast.AST) -> ir.SSAValue | None:
        raise BuildError(f"unreachable reached for {node.__class__.__name__}")


class NoSpecialLowering(FromPythonAST):
    pass
