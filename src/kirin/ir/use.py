from __future__ import annotations

from typing import TYPE_CHECKING
from dataclasses import dataclass

if TYPE_CHECKING:
    from kirin.ir.nodes.stmt import Statement


@dataclass(frozen=True)
class Use:
    """A use of an SSA value in a statement."""

    stmt: Statement
    """The statement that uses the SSA value."""
    index: int
    """The index of the use in the statement."""
