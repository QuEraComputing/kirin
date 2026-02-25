# NOTE: this module is only interface, will be used inside
# the `ir` module try to minimize the dependencies as much
# as possible

from __future__ import annotations

import ast
import inspect
from abc import ABC
from typing import TYPE_CHECKING, Any, Callable, ClassVar, TypeAlias
from dataclasses import dataclass

from kirin.ir.attrs import types
from kirin.lowering.abc import Result
from kirin.lowering.exception import BuildError

if TYPE_CHECKING:
    from kirin.lowering.state import State


LoweringTransform: TypeAlias = Callable[[Any, "State[ast.AST]", ast.Call], Result]


@dataclass
class Transform:
    objs: tuple[Callable, ...]
    func: LoweringTransform


@dataclass
class akin:
    obj: Callable

    def __call__(
        self,
        func: LoweringTransform | Transform,
    ) -> Transform:
        if isinstance(func, Transform):
            return Transform((self.obj,) + func.objs, func.func)

        func_name = getattr(func, "__name__", "")
        if not func_name.startswith("lower_Call_"):
            raise SyntaxError(
                "lowering function should be prefixed with lower_Call_"
                f" but got {func_name}"
            )
        return Transform((self.obj,), func)


@dataclass
class FromPythonAST(ABC):
    callee_table: ClassVar[dict[object, Transform]]
    """a table of lowering transforms for ast.Call based
    on the callable object if avaiable as a global value.
    """

    def __init_subclass__(cls) -> None:
        # init the subclass first
        super().__init_subclass__()
        cls.callee_table = {}
        for _, value in inspect.getmembers(cls):
            if isinstance(value, Transform):
                for obj in value.objs:
                    cls.callee_table[obj] = value

    @property
    def names(self) -> list[str]:  # show the name without lower_
        return [name[6:] for name in dir(self) if name.startswith("lower_")]

    def lower(self, state: State[ast.AST], node: ast.AST) -> Result:
        """Entry point of dialect specific lowering."""
        return getattr(self, f"lower_{node.__class__.__name__}", self.unreachable)(
            state, node
        )

    def unreachable(self, state: State[ast.AST], node: ast.AST) -> Result:
        raise BuildError(f"unreachable reached for {node.__class__.__name__}")

    @staticmethod
    def _flatten_hint_binop(node: ast.expr) -> list[ast.expr]:
        """Flatten a binary operation tree into a list of expressions.

        This is useful for handling union types represented as binary operations.
        """
        hints = []

        def _recurse(n: ast.expr):
            if isinstance(n, ast.BinOp):
                _recurse(n.left)
                _recurse(n.right)
            else:
                hints.append(n)

        _recurse(node)
        return hints

    @staticmethod
    def get_hint(state: State[ast.AST], node: ast.expr | None) -> types.TypeAttribute:
        if node is None:
            return types.AnyType()

        # deal with union syntax
        if isinstance(node, ast.BinOp):
            hint_nodes = FromPythonAST._flatten_hint_binop(node)
            hint_ts = []
            for i in range(len(hint_nodes)):
                hint_ts.append(
                    FromPythonAST.get_hint(
                        state,
                        hint_nodes[i],
                    )
                )
            return types.Union(hint_ts)

        try:
            t = state.get_global(node).data

            return types.hint2type(t)
        except Exception as e:  # noqa: E722
            raise BuildError(f"expect a type hint, got {ast.unparse(node)}") from e


class NoSpecialLowering(FromPythonAST):
    pass
