from __future__ import annotations

import ast
import inspect
import textwrap
from types import ModuleType
from typing import Any, Callable, Iterable
from dataclasses import dataclass

from kirin import ir
from kirin.source import SourceInfo
from kirin.registry import LoweringRegistry
from kirin.exception import StaticCheckError
from kirin.lowering.abc import Result, LoweringABC
from kirin.lowering.state import State
from kirin.lowering.exception import BuildError

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

    registry: LoweringRegistry

    def __init__(
        self,
        dialects: ir.DialectGroup | Iterable[ir.Dialect | ModuleType],
        *,
        keys: list[str] | None = None,
    ):
        if isinstance(dialects, ir.DialectGroup):
            self.dialects = dialects
        else:
            self.dialects = ir.DialectGroup(dialects)
        self.registry = self.dialects.registry.ast(keys=keys or ["main", "default"])

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
        region = self.run(
            ast.parse(source).body[0],
            source=source,
            globals=globals,
            file=file,
            lineno_offset=lineno_offset,
            col_offset=col_offset,
            compactify=compactify,
        )
        if not region.blocks:
            raise ValueError("No block generated")

        code = region.blocks[0].first_stmt
        if code is None:
            raise ValueError("No code generated")
        return code

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
    ) -> ir.Region:
        source = source or ast.unparse(stmt)
        state = State(
            self,
            source=SourceInfo.from_ast(stmt, file),
            file=file,
            lines=source.splitlines(),
            lineno_offset=lineno_offset,
            col_offset=col_offset,
        )

        with state.frame([stmt], globals=globals) as frame:
            try:
                self.visit(state, stmt)
            except StaticCheckError as e:
                if e.source is None:
                    e.source = state.source
                elif e.source.file is None:
                    e.source.file = state.file

                if e.source:
                    e.source.offset(lineno_offset, col_offset)
                e.lines = state.lines
                raise e

            region = frame.curr_region

        if compactify:
            from kirin.rewrite import Walk, CFGCompactify

            Walk(CFGCompactify()).rewrite(region)
        return region

    def lower_literal(self, state: State[ast.AST], value) -> ir.SSAValue:
        return state.lower(ast.Constant(value=value)).expect_one()

    def lower_global(self, state: State[ast.AST], node: ast.AST) -> LoweringABC.Result:
        return LoweringABC.Result(GlobalExprEval(state.current_frame).visit(node))

    # Python AST visitor methods
    def visit(self, state: State[ast.AST], node: ast.AST) -> Result:
        if hasattr(node, "lineno"):
            state.source = SourceInfo.from_ast(node, state.file)
        name = node.__class__.__name__
        if name in self.registry.ast_table:
            return self.registry.ast_table[name].lower(state, node)
        return getattr(self, f"visit_{name}", self.generic_visit)(state, node)

    def generic_visit(self, state: State[ast.AST], node: ast.AST) -> Result:
        raise BuildError(f"Cannot lower {node.__class__.__name__} node: {node}")

    def visit_Call(self, state: State[ast.AST], node: ast.Call) -> Result:
        if hasattr(node.func, "lineno"):
            state.source = SourceInfo.from_ast(node.func, state.file)

        global_callee_result = state.get_global(node.func, no_raise=True)
        if global_callee_result is None:
            return self.visit_Call_local(state, node)

        global_callee = global_callee_result.data

        if isinstance(global_callee, Binding):
            global_callee = global_callee.parent

        if isinstance(global_callee, ir.Method):
            return self.visit_Call_Method(state, node, global_callee)
        elif inspect.isclass(global_callee) and issubclass(global_callee, ir.Statement):
            return self.visit_Call_Class_Statement(state, node, global_callee)
        else:
            return self.visit_Call_table(state, node, global_callee)

    def visit_Call_table(self, state: State[ast.AST], node: ast.Call, global_callee):
        if method := self.registry.callee_table.get(global_callee):
            return method(state, node)
        return self.visit_Call_generic(state, node, global_callee)

    def visit_Call_generic(self, state: State[ast.AST], node: ast.Call, global_callee):
        # symbol exist in global, but not ir.Statement, it may refer to a
        # local value that shadows the global value
        try:
            return self.visit_Call_local(state, node)
        except BuildError:
            # symbol exist in global, but not ir.Statement, not found in locals either
            # this means the symbol is referring to an external uncallable object
            # try to hint the user
            if inspect.isfunction(global_callee) or inspect.ismethod(global_callee):
                raise BuildError(
                    f"unsupported callee: {repr(global_callee)}."
                    "Are you trying to call a python function? This is not supported."
                )
            else:  # well not much we can do, can't hint
                raise BuildError(
                    f"unsupported call to {repr(global_callee)}, "
                    "expected a kernel function (Method), "
                    "wrapped statement (Binding) or a supported Python function"
                    f", got {type(global_callee)}"
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
        if "Call_global_method" in self.registry.ast_table:
            return self.registry.ast_table[
                "Call_global_method"
            ].lower_Call_global_method(state, global_callee, node)
        raise BuildError("`lower_Call_global_method` not implemented")

    def visit_Call_local(self, state: State[ast.AST], node: ast.Call) -> Result:
        callee = state.lower(node.func).expect_one()
        if "Call_local" in self.registry.ast_table:
            return self.registry.ast_table["Call_local"].lower_Call_local(
                state, callee, node
            )
        raise BuildError("`lower_Call_local` not implemented")

    def visit_With(self, state: State[ast.AST], node: ast.With) -> Result:
        if len(node.items) != 1:
            raise BuildError("expected exactly one item in with statement")

        item = node.items[0]
        if not isinstance(item.context_expr, ast.Call):
            raise BuildError("expected context expression to be a call")

        global_callee = state.get_global(item.context_expr.func).data
        if isinstance(global_callee, Binding):
            global_callee = global_callee.parent

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
