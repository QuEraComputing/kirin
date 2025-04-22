from typing import TypeVar

from kirin import ir, interp

from .abc import EmitABC, EmitFrame

ValueType = TypeVar("ValueType")
FrameType = TypeVar("FrameType", bound=interp.Frame)


class Transform(EmitABC[FrameType, ir.IRNode]):
    pass
