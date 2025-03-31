from __future__ import annotations

import ast
import inspect
import textwrap
from types import ModuleType
from typing import Any, Callable, Iterable
from dataclasses import dataclass

from kirin import ir
from kirin.source import SourceInfo

from .abc import LoweringABC
from .state import State
from .dialect import FromPythonAST
from .exception import DialectLoweringError


@dataclass
class PythonLowering(LoweringABC[ast.stmt | Callable]):
    registry: dict[str, FromPythonAST]
    max_lines: int = 3

    def __init__(
        self,
        dialects: ir.DialectGroup | Iterable[ir.Dialect | ModuleType],
        keys: list[str] | None = None,
        max_lines: int = 3,
    ):
        if isinstance(dialects, ir.DialectGroup):
            self.dialects = dialects
        else:
            self.dialects = ir.DialectGroup(dialects)

        self.max_lines = max_lines
        self.registry = self.dialects.registry.ast(keys=keys or ["main", "default"])

    def run(
        self,
        stmt: ast.stmt | Callable[..., Any],
        source: str | None = None,
        globals: dict[str, Any] | None = None,
        lineno_offset: int = 0,
        col_offset: int = 0,
        compactify: bool = True,
    ) -> ir.Statement:
        if isinstance(stmt, Callable):
            source = source or textwrap.dedent(inspect.getsource(stmt))
            globals = globals or stmt.__globals__
            try:
                nonlocals = inspect.getclosurevars(stmt).nonlocals
            except Exception:
                nonlocals = {}
            globals.update(nonlocals)
            stmt = ast.parse(source).body[0]

        source = source or ast.unparse(stmt)
        state = State(
            self,
            source=SourceInfo.from_ast(stmt, lineno_offset, col_offset),
            lines=source.splitlines(),
            lineno_offset=lineno_offset,
            col_offset=col_offset,
        )

        try:
            self.visit(state, stmt)
        except DialectLoweringError as e:
            e.args = (f"{e.args[0]}\n\n{self.error_hint()}",) + e.args[1:]
            raise e

        if compactify:
            from kirin.rewrite import Walk, CFGCompactify

            Walk(CFGCompactify()).rewrite(state.code)
        return state.code

    def lower_Constant(self, state: State, value) -> ir.SSAValue:
        value = self.visit(state, ast.Constant(value=value))
        if value is None:
            raise DialectLoweringError("Cannot lower constant value to IR")
        return value

    def visit(self, state: State, node: ast.AST) -> ir.SSAValue | None:
        self.source = SourceInfo.from_ast(node, state.lineno_offset, state.col_offset)
        name = node.__class__.__name__
        if name in self.registry:
            return self.registry[name].lower(state, node)
        return getattr(self, f"visit_{name}", self.generic_visit)(state, node)

    def generic_visit(self, state: State, node: ast.AST) -> ir.SSAValue | None:
        raise DialectLoweringError(
            f"Cannot lower {node.__class__.__name__} node: {node}"
        )

    def visit_Call(self, state: State, node: ast.Call) -> ir.SSAValue | None: ...

    def visit_With(self, state: State, node: ast.With) -> ir.SSAValue | None: ...

    def error_hint(self) -> str: ...
