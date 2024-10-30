import ast
from dataclasses import dataclass


@dataclass
class SourceInfo:
    lineno: int
    col_offset: int
    end_lineno: int | None
    end_col_offset: int | None

    @classmethod
    def from_ast(cls, node: ast.AST, lineno_offset: int = 0, col_offset: int = 0):
        end_lineno = getattr(node, "end_lineno", None)
        end_col_offset = getattr(node, "end_col_offset", None)
        return cls(
            getattr(node, "lineno", 0) + lineno_offset,
            getattr(node, "col_offset", 0) + col_offset,
            end_lineno + lineno_offset if end_lineno is not None else None,
            end_col_offset + col_offset if end_col_offset is not None else None,
        )
