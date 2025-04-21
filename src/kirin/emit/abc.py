from abc import ABC
from typing import TypeVar
from dataclasses import field, dataclass

from kirin import ir, interp

ValueType = TypeVar("ValueType")


@dataclass
class EmitFrame(interp.Frame[ValueType]):
    block_ref: dict[ir.Block, ValueType] = field(default_factory=dict)


FrameType = TypeVar("FrameType", bound=EmitFrame)


@dataclass
class EmitABC(interp.BaseInterpreter[FrameType, ValueType], ABC):

    def emit(self, code: ir.Statement | ir.Method) -> ValueType:
        if isinstance(code, ir.Method):
            code = code.code

        with self.new_frame(code) as frame:
            result = self.eval_stmt(frame, code)
            if result is None:
                return self.void
            elif isinstance(result, tuple) and len(result) == 1:
                return result[0]
            raise interp.InterpreterError(
                f"Unexpected result {result} from statement {code.name}"
            )

    def emit_attribute(self, attr: ir.Attribute) -> ValueType:
        if attr.dialect not in self.dialects:
            raise interp.InterpreterError(
                f"Attribute {attr} not in dialects {self.dialects}"
            )

        return getattr(
            self, f"emit_type_{type(attr).__name__}", self.emit_attribute_fallback
        )(attr)

    def emit_attribute_fallback(self, attr: ir.Attribute) -> ValueType:
        if (method := self.registry.attributes.get(type(attr))) is not None:
            return method(self, attr)
        raise NotImplementedError(f"Attribute {type(attr)} not implemented")
