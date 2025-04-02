from __future__ import annotations

import ast
import builtins
from typing import Any
from dataclasses import dataclass

from kirin.lowering.frame import Frame
from kirin.lowering.exception import BuildError


class GlobalEvalError(BuildError):
    """Exception raised when a global expression cannot be evaluated."""

    pass


@dataclass
class GlobalExprEval(ast.NodeVisitor):
    frame: Frame

    def generic_visit(self, node: ast.AST) -> Any:
        raise GlobalEvalError(
            f"Cannot lower global {node.__class__.__name__} node: {ast.dump(node)}"
        )

    def visit_Name(self, node: ast.Name) -> Any:
        if not isinstance(node.ctx, ast.Load):
            raise GlobalEvalError("unsupported name access")

        name = node.id
        value = self.frame.globals.get(name)
        if value is not None:
            return value

        if hasattr(builtins, name):
            return getattr(builtins, name)
        else:
            raise GlobalEvalError(f"global {name} not found")

    def visit_Constant(self, node: ast.Constant) -> Any:
        return node.value

    def visit_Attribute(self, node: ast.Attribute) -> Any:
        if not isinstance(node.ctx, ast.Load):
            raise GlobalEvalError("unsupported attribute access")

        value = self.visit(node.value)
        if hasattr(value, node.attr):
            return getattr(value, node.attr)

        raise GlobalEvalError(f"attribute {node.attr} not found in {value}")

    def visit_Subscript(self, node: ast.Subscript) -> Any:
        value = self.visit(node.value)
        if not hasattr(value, "__getitem__"):
            raise GlobalEvalError(
                f"unsupported subscript access for class {type(value)}"
            )

        return value[self.visit(node.slice)]

    def visit_Call(self, node: ast.Call) -> Any:
        func = self.visit(node.func)
        args = [self.visit(arg) for arg in node.args]
        keywords = {
            kw.arg: self.visit(kw) for kw in node.keywords if kw.arg is not None
        }
        if not callable(func):
            raise GlobalEvalError(f"global object {func} is not callable")

        try:
            return func(*args, **keywords)
        except TypeError as e:
            raise GlobalEvalError(
                f"TypeError in global call: {e} for {func}({args}, {keywords})"
            ) from e
        except Exception as e:
            raise GlobalEvalError(
                f"Exception in global call: {e} for {func}({args}, {keywords})"
            ) from e

    def visit_Tuple(self, node: ast.Tuple) -> Any:
        return tuple(self.visit(elt) for elt in node.elts)
