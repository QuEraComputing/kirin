from typing import Generic, TypeVar
from contextlib import contextmanager
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

    frames: list[FrameType] = field(default_factory=list)

    def push_frame(self, frame: FrameType):
        """Push a frame onto the stack.

        Args:
            frame(FrameType): The frame to push onto the stack.

        Returns:
            FrameType: The frame that was pushed.
        """
        self.frames.append(frame)
        return frame

    def pop_frame(self) -> FrameType:
        """Pop a frame from the stack.

        Returns:
            FrameType: The frame that was popped.
        """
        return self.frames.pop()

    @contextmanager
    def new_frame(self, frame: FrameType):
        """Context manager to push and pop a frame.

        This context manager is used to push a frame onto the stack before
        executing a block of code and pop the frame from the stack after
        executing the block of code.

        Args:
            frame(FrameType): The frame to push onto the stack.

        Yields:
            FrameType: The frame that was pushed.
        """
        self.push_frame(frame)
        try:
            yield frame
        finally:
            self.pop_frame()

    def current_frame(self) -> FrameType:
        """Get the current frame.

        Returns:
            FrameType: The current frame.
        """
        if not self.frames:
            raise ValueError(
                "unable to retrieve the current frame because no frames were pushed"
            )
        return self.frames[-1]
