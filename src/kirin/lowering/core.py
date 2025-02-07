import ast
import inspect
import textwrap
from types import ModuleType
from typing import Any, Callable, Iterable
from dataclasses import dataclass

from kirin.ir import Dialect, DialectGroup
from kirin.exceptions import DialectLoweringError
from kirin.lowering.state import LoweringState
from kirin.lowering.dialect import FromPythonAST


@dataclass(init=False)
class Lowering(ast.NodeVisitor):
    dialects: DialectGroup
    registry: dict[str, FromPythonAST]
    state: LoweringState | None = None

    # max lines to show in error hint
    max_lines: int = 3

    def __init__(
        self,
        dialects: DialectGroup | Iterable[Dialect | ModuleType],
        keys: list[str] | None = None,
        max_lines: int = 3,
    ):
        if isinstance(dialects, DialectGroup):
            self.dialects = dialects
        else:
            self.dialects = DialectGroup(dialects)

        self.max_lines = max_lines
        self.registry: dict[str, FromPythonAST] = self.dialects.registry.ast(
            keys=keys or ["main", "default"]
        )
        self.state = None

    def run(
        self,
        stmt: ast.stmt | Callable,
        source: str | None = None,
        globals: dict[str, Any] | None = None,
        lineno_offset: int = 0,
        col_offset: int = 0,
        compactify: bool = True,
    ):
        if isinstance(stmt, Callable):
            source = source or textwrap.dedent(inspect.getsource(stmt))
            globals = globals or stmt.__globals__
            try:
                nonlocals = inspect.getclosurevars(stmt).nonlocals
            except Exception:
                nonlocals = {}
            globals.update(nonlocals)
            stmt = ast.parse(source).body[0]

        state = LoweringState.from_stmt(
            self, stmt, source, globals, self.max_lines, lineno_offset, col_offset
        )
        try:
            state.visit(stmt)
        except DialectLoweringError as e:
            e.args = (f"{e.args[0]}\n\n{state.error_hint()}",) + e.args[1:]
            raise e

        if compactify:
            from kirin.rewrite import Walk, CFGCompactify

            Walk(CFGCompactify()).rewrite(state.code)
        return state.code
