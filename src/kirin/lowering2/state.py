from __future__ import annotations

from typing import TYPE_CHECKING
from dataclasses import field, dataclass

from kirin.source import SourceInfo

from .frame import Frame

if TYPE_CHECKING:
    from .abc import LoweringABC


@dataclass
class State:
    """State of the lowering process.

    This class is used to store the state of the lowering process.
    It contains the current frame, the current block, and
    the current source.
    """

    parent: LoweringABC
    """the parent lowering transform"""
    source: SourceInfo
    "source info of the current node"
    lines: list[str]
    "source lines of the code being lowered"
    lineno_offset: int = 0
    "lineno offset at the beginning of the source"
    col_offset: int = 0
    "column offset at the beginning of the source"
    _current_frame: Frame | None = field(default=None, init=False, repr=False)
    "current frame being lowered"

    def __repr__(self) -> str:
        return f"lowering.State({self.current_frame})"

    @property
    def code(self):
        region = self.root_frame.curr_region
        if not region.blocks:
            raise ValueError("No block generated")

        stmt = region.blocks[0].first_stmt
        if stmt is None:
            raise ValueError("No code generated")
        return stmt

    @property
    def root_frame(self) -> Frame:
        """root frame of the lowering process"""
        if self._current_frame is None:
            raise ValueError("current frame is None")
        root = self._current_frame
        while root.parent is not None:
            root = root.parent
        return root

    @property
    def current_frame(self) -> Frame:
        """current frame being lowered"""
        if self._current_frame is None:
            raise ValueError("current frame is None")
        return self._current_frame

    def push_frame(self, frame: Frame):
        frame.parent = self._current_frame
        self._current_frame = frame
        return frame

    def pop_frame(self) -> Frame:
        if self._current_frame is None:
            raise ValueError("current frame is None")
        frame = self._current_frame
        self._current_frame = frame.parent
        return frame
