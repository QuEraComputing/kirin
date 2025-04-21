import ast
from dataclasses import dataclass


@dataclass
class SourceInfo:
    lineno: int
    col_offset: int
    end_lineno: int | None
    end_col_offset: int | None
    file: str | None = None
    lineno_begin: int = 0
    col_indent: int = 0

    @classmethod
    def from_ast(
        cls,
        node: ast.AST,
        file: str | None = None,
    ):
        end_lineno = getattr(node, "end_lineno", None)
        end_col_offset = getattr(node, "end_col_offset", None)
        return cls(
            getattr(node, "lineno", 0),
            getattr(node, "col_offset", 0),
            end_lineno if end_lineno is not None else None,
            end_col_offset if end_col_offset is not None else None,
            file,
        )

    def offset(self, lineno_begin: int = 0, col_indent: int = 0):
        """Offset the source info by the given offsets.

        Args:
            lineno_offset (int): The line number offset.
            col_offset (int): The column offset.
        """
        self.lineno_begin = lineno_begin
        self.col_indent = col_indent

    def __repr__(self) -> str:
        return (
            f'File "{self.file or "stdin"}", '
            f"line {self.lineno + self.lineno_begin},"
            f" col {self.col_offset + self.col_indent}"
        )
