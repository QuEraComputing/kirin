from __future__ import annotations

import ast
import inspect
import textwrap
from types import ModuleType
from typing import Any, Callable, Iterable
from dataclasses import dataclass

from rich.console import Console

from kirin import ir
from kirin.source import SourceInfo
from kirin.lowering.abc import Result, LoweringABC
from kirin.lowering.state import State
from kirin.lowering.exception import BuildError
from kirin.lowering.python.dialect import FromPythonAST

from .glob import GlobalExprEval
from .traits import FromPythonCall, FromPythonWithSingleItem
from .binding import Binding


@dataclass
class Python(LoweringABC[ast.AST]):
    """Python lowering transform.

    This class is used to lower Python AST nodes to IR statements via
    a visitor pattern.

    !!! note
        the visitor pattern is not using the `ast.NodeVisitor` class
        because it customize the visit method to pass the lowering state
        and the source information to the visitor methods.
    """

    registry: dict[str, FromPythonAST]
    max_lines: int = 3
    hint_indent: int = 2
    hint_lineno: bool = True
    stacktrace: bool = False
    """If True, print the stacktrace of the error."""

    def __init__(
        self,
        dialects: ir.DialectGroup | Iterable[ir.Dialect | ModuleType],
        *,
        keys: list[str] | None = None,
        max_lines: int = 3,
        hint_indent: int = 2,
        hint_lineno: bool = True,
        stacktrace: bool = False,
    ):
        if isinstance(dialects, ir.DialectGroup):
            self.dialects = dialects
        else:
            self.dialects = ir.DialectGroup(dialects)

        self.max_lines = max_lines
        self.registry = self.dialects.registry.ast(keys=keys or ["main", "default"])
        self.hint_indent = hint_indent
        self.hint_lineno = hint_lineno
        self.stacktrace = stacktrace

    def python_function(
        self,
        func: Callable,
        *,
        globals: dict[str, Any] | None = None,
        lineno_offset: int = 0,
        col_offset: int = 0,
        compactify: bool = True,
    ):
        file = inspect.getfile(func)
        source = textwrap.dedent(inspect.getsource(func))
        if globals:
            globals.update(func.__globals__)
        else:
            globals = func.__globals__

        try:
            nonlocals = inspect.getclosurevars(func).nonlocals
        except Exception:
            nonlocals = {}
        globals.update(nonlocals)
        return self.run(
            ast.parse(source).body[0],
            source=source,
            globals=globals,
            file=file,
            lineno_offset=lineno_offset,
            col_offset=col_offset,
            compactify=compactify,
        )

    def run(
        self,
        stmt: ast.AST,
        *,
        source: str | None = None,
        globals: dict[str, Any] | None = None,
        file: str | None = None,
        lineno_offset: int = 0,
        col_offset: int = 0,
        compactify: bool = True,
    ) -> ir.Statement:
        source = source or ast.unparse(stmt)
        state = State(
            self,
            source=SourceInfo.from_ast(stmt, lineno_offset, col_offset),
            file=file,
            lines=source.splitlines(),
            lineno_offset=lineno_offset,
            col_offset=col_offset,
        )

        with state.frame([stmt], parent=None, globals=globals) as frame:
            try:
                self.visit(state, stmt)
            except BuildError as e:
                if self.stacktrace:
                    raise Exception(
                        f"{e.args[0]}\n\n{self.error_hint(state, e)}",
                        *e.args[1:],
                    ) from e
                else:
                    e.args = (self.error_hint(state, e),)
                    raise e

            region = frame.curr_region
            if not region.blocks:
                raise ValueError("No block generated")

            code = region.blocks[0].first_stmt
            if code is None:
                raise ValueError("No code generated")

        if compactify:
            from kirin.rewrite import Walk, CFGCompactify

            Walk(CFGCompactify()).rewrite(code)
        return code

    def lower_literal(self, state: State[ast.AST], value) -> ir.SSAValue:
        return state.lower(ast.Constant(value=value)).expect_one()

    def lower_global(self, state: State[ast.AST], node: ast.AST) -> LoweringABC.Result:
        return LoweringABC.Result(GlobalExprEval(state.current_frame).visit(node))

    # Python AST visitor methods
    def visit(self, state: State[ast.AST], node: ast.AST) -> Result:
        if hasattr(node, "lineno"):
            state.source = SourceInfo.from_ast(
                node, state.lineno_offset, state.col_offset
            )
        name = node.__class__.__name__
        if name in self.registry:
            return self.registry[name].lower(state, node)
        return getattr(self, f"visit_{name}", self.generic_visit)(state, node)

    def generic_visit(self, state: State[ast.AST], node: ast.AST) -> Result:
        raise BuildError(f"Cannot lower {node.__class__.__name__} node: {node}")

    def visit_Call(self, state: State[ast.AST], node: ast.Call) -> Result:
        if hasattr(node.func, "lineno"):
            state.source = SourceInfo.from_ast(
                node.func, state.lineno_offset, state.col_offset
            )

        global_callee_result = self.lower_global_no_raise(state, node.func)
        if global_callee_result is None:
            return self.visit_Call_local(state, node)

        global_callee = global_callee_result.data

        if isinstance(global_callee, Binding):
            global_callee = global_callee.parent

        if isinstance(global_callee, ir.Method):
            return self.visit_Call_Method(state, node, global_callee)
        elif inspect.isclass(global_callee):
            return self.visit_Call_Class(state, node, global_callee)
        elif inspect.isbuiltin(global_callee):
            name = f"Call_{global_callee.__name__}"
            if "Call_builtins" in self.registry:
                dialect_lowering = self.registry["Call_builtins"]
                return dialect_lowering.lower_Call_builtins(state, node)
            elif name in self.registry:
                dialect_lowering = self.registry[name]
                return getattr(dialect_lowering, f"lower_{name}")(state, node)
            else:
                raise BuildError(
                    f"`lower_{name}` is not implemented for builtin function `{global_callee.__name__}`."
                )

        # symbol exist in global, but not ir.Statement, it may refer to a
        # local value that shadows the global value
        try:
            return self.visit_Call_local(state, node)
        except BuildError:
            # symbol exist in global, but not ir.Statement, not found in locals either
            # this means the symbol is referring to an external uncallable object
            # try to hint the user
            if inspect.isfunction(global_callee):
                raise BuildError(
                    f"unsupported callee: {repr(global_callee)}."
                    "Are you trying to call a python function? This is not supported."
                )
            else:  # well not much we can do, can't hint
                raise BuildError(f"unsupported callee type: {repr(global_callee)}")

    def visit_Call_Class(
        self, state: State[ast.AST], node: ast.Call, global_callee: type
    ):
        if issubclass(global_callee, ir.Statement):
            return self.visit_Call_Class_Statement(state, node, global_callee)
        else:
            return getattr(
                self,
                f"visit_Call_Class_{global_callee.__name__}",
                self.visit_Call_Class_generic,
            )(state, node, global_callee)

    def visit_Call_Class_generic(
        self, state: State[ast.AST], node: ast.Call, global_callee: type
    ) -> Result:
        if f"Call_{global_callee.__name__}" in self.registry:
            methods = self.registry[f"Call_{global_callee.__name__}"]
            return getattr(methods, f"lower_Call_{global_callee.__name__}")(state, node)
        raise BuildError(
            f"`lower_Call_{global_callee.__name__}` not implemented by any dialect"
            f" for {global_callee.__name__}"
        )

    def visit_Call_Class_Statement(
        self, state: State[ast.AST], node: ast.Call, global_callee: type[ir.Statement]
    ):
        if global_callee.dialect is None:
            raise BuildError(f"unsupported dialect `None` for {global_callee.name}")

        if global_callee.dialect not in self.dialects.data:
            raise BuildError(f"unsupported dialect `{global_callee.dialect.name}`")

        if (trait := global_callee.get_trait(FromPythonCall)) is not None:
            return trait.lower(global_callee, state, node)
        raise BuildError(
            f"invalid call syntax for {global_callee.__name__}, "
            f"expected FromPythonCall trait to be implemented"
            f" for {global_callee.__name__}"
        )

    def visit_Call_Method(
        self, state: State[ast.AST], node: ast.Call, global_callee: ir.Method
    ) -> Result:
        if "Call_global_method" in self.registry:
            return self.registry["Call_global_method"].lower_Call_global_method(
                state, global_callee, node
            )
        raise BuildError("`lower_Call_global_method` not implemented")

    def visit_Call_local(self, state: State[ast.AST], node: ast.Call) -> Result:
        callee = state.lower(node.func).expect_one()
        if "Call_local" in self.registry:
            return self.registry["Call_local"].lower_Call_local(state, callee, node)
        raise BuildError("`lower_Call_local` not implemented")

    def visit_With(self, state: State[ast.AST], node: ast.With) -> Result:
        if len(node.items) != 1:
            raise BuildError("expected exactly one item in with statement")

        item = node.items[0]
        if not isinstance(item.context_expr, ast.Call):
            raise BuildError("expected context expression to be a call")

        global_callee = state.get_global(item.context_expr.func).data
        if not issubclass(global_callee, ir.Statement):
            raise BuildError(
                f"expected context expression to be a statement, got {global_callee}"
            )

        if trait := global_callee.get_trait(FromPythonWithSingleItem):
            return trait.lower(global_callee, state, node)

        raise BuildError(
            f"invalid with syntax for {global_callee.__name__}, "
            f"expected FromPythonWithSingleItem trait"
            " to be implemented"
            f" for {global_callee.__name__}"
        )

    def error_hint(self, state: State[ast.AST], err: BuildError) -> str:
        begin = max(0, state.source.lineno - self.max_lines - state.lineno_offset)
        end = max(
            max(state.source.lineno + self.max_lines, state.source.end_lineno or 0)
            - state.lineno_offset,
            0,
        )
        end = min(len(state.lines), end)  # make sure end is within bounds
        lines = state.lines[begin:end]
        error_lineno = state.source.lineno - state.lineno_offset - 1
        error_lineno_len = len(str(state.source.lineno))
        code_indent = min(map(self.__get_indent, lines), default=0)

        console = Console(force_terminal=True)
        with console.capture() as capture:
            console.print()
            console.print(
                f"[dim]{state.file or 'stdin'}:{state.source.lineno}[/dim]",
                markup=True,
                highlight=False,
            )
            emsg = "\n  ".join(err.args)
            console.print(f"[red]  {type(err).__name__}: {emsg}[/red]")
            for lineno, line in enumerate(lines, begin):
                line = " " * self.hint_indent + line[code_indent:]
                if self.hint_lineno:
                    if lineno == error_lineno:
                        line = f"{state.source.lineno}[dim]│[/dim]" + line
                    else:
                        line = "[dim] " * (error_lineno_len) + "│[/dim]" + line
                console.print("  " + line, markup=True, highlight=False)
                if lineno == error_lineno:
                    console.print(
                        "  "
                        + self.__hint_line(
                            state, code_indent, error_lineno_len, err.hint
                        ),
                        markup=True,
                        highlight=False,
                    )

            if end == error_lineno:
                console.print(
                    "  "
                    + self.__hint_line(state, code_indent, error_lineno_len, err.hint),
                    markup=True,
                    highlight=False,
                )

        return capture.get()

    def __hint_line(
        self, state: State[ast.AST], code_indent: int, error_lineno_len: int, msg: str
    ) -> str:
        hint = " " * (state.source.col_offset - code_indent)
        if state.source.end_col_offset:
            hint += "^" * (state.source.end_col_offset - state.source.col_offset)
        else:
            hint += "^"

        hint = " " * self.hint_indent + "[red]" + hint + " error: " + msg + "[/red]"
        if self.hint_lineno:
            hint = " " * error_lineno_len + "[dim]│[/dim]" + hint
        return hint

    @staticmethod
    def __get_indent(line: str) -> int:
        if len(line) == 0:
            return int(1e9)  # very large number
        return len(line) - len(line.lstrip())
