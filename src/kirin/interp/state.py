from typing import Generic, TypeVar
from dataclasses import field, dataclass

from kirin.interp.frame import FrameABC

FrameType = TypeVar("FrameType", bound=FrameABC)


@dataclass
class InterpreterState(Generic[FrameType]):
    frames: list[FrameType] = field(default_factory=list)

    def push_frame(self, frame: FrameType):
        self.frames.append(frame)
        return frame

    def pop_frame(self) -> FrameType:
        return self.frames.pop()

    def current_frame(self) -> FrameType:
        if not self.frames:
            raise ValueError(
                "unable to retrieve the current frame because no frames were pushed"
            )
        return self.frames[-1]

    def stacktrace(self) -> list[FrameType]:
        return self.frames
