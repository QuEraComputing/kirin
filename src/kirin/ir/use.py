from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from kirin.ir.nodes.stmt import Statement


@dataclass(frozen=True)
class Use:
    stmt: Statement
    index: int
