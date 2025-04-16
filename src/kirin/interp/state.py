from typing import Generic, TypeVar
from dataclasses import field, dataclass

from kirin.interp.frame import FrameABC

FrameType = TypeVar("FrameType", bound=FrameABC)


@dataclass
class InterpreterState(Generic[FrameType]):
    """Interpreter state.

    This class represents the state of the interpreter. It contains the
    stack of frames for the interpreter. The stack of frames is used to
    store the current state of the interpreter during interpretation.
    """

    _current_frame: FrameType | None = field(
        default=None, kw_only=True, init=False, repr=False
    )
    """Current frame of the stack."""
    depth: int = field(default=0, kw_only=True, init=False, repr=False)
    """stack depth of the interpreter."""

    @property
    def current_frame(self) -> FrameType:
        """Get the current frame.

        Returns:
            FrameType: The current frame.
        """
        if self._current_frame is None:
            raise ValueError("no current frame")
        return self._current_frame

    def push_frame(self, frame: FrameType) -> FrameType:
        """Push a frame onto the stack.

        Args:
            frame(FrameType): The frame to push onto the stack.

        Returns:
            FrameType: The frame that was pushed.
        """
        assert frame.parent is None, "frame already has a parent"
        self.depth += 1
        if self._current_frame is None:
            self._current_frame = frame
        else:
            frame.parent = self._current_frame
            self._current_frame = frame
        return self._current_frame

    def pop_frame(self) -> FrameType:
        """Pop a frame from the stack.

        Returns:
            FrameType: The frame that was popped.
        """
        if self._current_frame is None:
            raise ValueError("no frame to pop")
        frame = self._current_frame
        self._current_frame = self._current_frame.parent
        self.depth -= 1
        frame.parent = None
        return frame
