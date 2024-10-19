from dataclasses import dataclass, field
from typing import Generic, TypeVar

from kirin.interp.frame import Frame

ValueType = TypeVar("ValueType")


@dataclass
class InterpreterState(Generic[ValueType]):
    frames: list[Frame[ValueType]] = field(default_factory=list)

    def push_frame(self, frame: Frame[ValueType]):
        self.frames.append(frame)
        return frame

    def pop_frame(self) -> Frame[ValueType]:
        return self.frames.pop()

    def current_frame(self) -> Frame[ValueType]:
        if not self.frames:
            raise ValueError(
                "unable to retrieve the current frame because no frames were pushed"
            )
        return self.frames[-1]

    def stacktrace(self) -> list[Frame[ValueType]]:
        return self.frames
