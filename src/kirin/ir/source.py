from dataclasses import dataclass


@dataclass
class SourceInfo:
    lineno: int
    col_offset: int
    end_lineno: int | None
    end_col_offset: int | None
