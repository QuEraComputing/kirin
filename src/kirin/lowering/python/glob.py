from __future__ import annotations

import ast
import builtins
from typing import Any
from dataclasses import dataclass

from kirin.source import SourceInfo
from kirin.lowering.frame import Frame
from kirin.lowering.exception import BuildError


class GlobalEvalError(BuildError):
    """Exception raised when a global expression cannot be evaluated."""

    def __init__(self, node: ast.AST, *msgs: str, help: str | None = None):
        super().__init__(*msgs, help=help)
        self.source = SourceInfo.from_ast(node)


@dataclass
class GlobalExprEval(ast.NodeVisitor):
    frame: Frame

    def generic_visit(self, node: ast.AST) -> Any:
        if isinstance(node, ast.AST):
            raise GlobalEvalError(
                node,
                f"Cannot lower global {node.__class__.__name__} node: {ast.dump(node)}",
            )
        raise GlobalEvalError(
            node,
            f"Unexpected global `{node.__class__.__name__}` node: {repr(node)} is not an AST node",
        )

    def visit_Name(self, node: ast.Name) -> Any:
        if not isinstance(node.ctx, ast.Load):
            raise GlobalEvalError(node, "unsupported name access")

        name = node.id
        value = self.frame.globals.get(name)
        if value is not None:
            return value

        if hasattr(builtins, name):
            return getattr(builtins, name)
        else:
            raise GlobalEvalError(node, f"global {name} not found")

    def visit_Constant(self, node: ast.Constant) -> Any:
        return node.value

    def visit_Attribute(self, node: ast.Attribute) -> Any:
        if not isinstance(node.ctx, ast.Load):
            raise GlobalEvalError(node, "unsupported attribute access")

        value = self.visit(node.value)
        if hasattr(value, node.attr):
            return getattr(value, node.attr)

        raise GlobalEvalError(node, f"attribute {node.attr} not found in {value}")

    def visit_Subscript(self, node: ast.Subscript) -> Any:
        value = self.visit(node.value)

        if isinstance(value, type):
            if hasattr(value, "__class_getitem__"):
                return value.__class_getitem__(self.visit(node.slice))
            else:
                raise GlobalEvalError(
                    node, f"class {value} does not support type parameters"
                )

        if not hasattr(value, "__getitem__"):
            raise GlobalEvalError(
                node, f"unsupported subscript access for class {type(value)}"
            )

        return value[self.visit(node.slice)]

    def visit_Call(self, node: ast.Call) -> Any:
        func = self.visit(node.func)
        args = [self.visit(arg) for arg in node.args]
        keywords = {
            kw.arg: self.visit(kw) for kw in node.keywords if kw.arg is not None
        }
        if not callable(func):
            raise GlobalEvalError(node, f"global object {func} is not callable")

        try:
            return func(*args, **keywords)
        except TypeError as e:
            raise GlobalEvalError(
                node, f"TypeError in global call: {e} for {func}({args}, {keywords})"
            ) from e
        except Exception as e:
            raise GlobalEvalError(
                node, f"Exception in global call: {e} for {func}({args}, {keywords})"
            ) from e

    def visit_Tuple(self, node: ast.Tuple) -> Any:
        return tuple(self.visit(elt) for elt in node.elts)

    def visit_List(self, node: ast.List) -> Any:
        return [self.visit(elt) for elt in node.elts]
