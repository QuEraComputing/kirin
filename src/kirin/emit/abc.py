from typing import TypeVar, Iterable
from dataclasses import field, dataclass

from kirin import ir, interp
from kirin.exceptions import FuelExhaustedError

ValueType = TypeVar("ValueType")


@dataclass
class EmitFrame(interp.Frame[ValueType]):
    block_ref: dict[ir.Block, ValueType] = field(default_factory=dict)


FrameType = TypeVar("FrameType", bound=EmitFrame)


class EmitABC(interp.BaseInterpreter[FrameType, ValueType]):

    def __init__(
        self,
        dialects: ir.DialectGroup | Iterable[ir.Dialect],
        bottom: ValueType,
        *,
        fuel: int | None = None,
        max_depth: int = 128,
        max_python_recursion_depth: int = 8192,
    ):
        super().__init__(
            dialects,
            bottom,
            fuel=fuel,
            max_depth=max_depth,
            max_python_recursion_depth=max_python_recursion_depth,
        )

    def run_callable_region(
        self, frame: FrameType, code: ir.Statement, region: ir.Region
    ) -> ValueType | interp.Err[ValueType]:
        results = self.run_stmt(frame, code)
        if isinstance(results, interp.Err):
            return results
        elif isinstance(results, tuple):
            if len(results) == 0:
                return self.bottom
            elif len(results) == 1:
                return results[0]
        raise ValueError(f"Unexpected results {results}")

    def run_ssacfg_region(
        self, frame: FrameType, region: ir.Region
    ) -> ValueType | interp.Err[ValueType]:
        result = self.bottom
        for block in region.blocks:
            block_header = self.emit_block(frame, block)
            if isinstance(block_header, interp.Err):
                return block_header
            frame.block_ref[block] = block_header

        return result

    def emit_attribute(self, attr: ir.Attribute) -> ValueType:
        return getattr(
            self, f"emit_type_{type(attr).__name__}", self.emit_attribute_fallback
        )(attr)

    def emit_attribute_fallback(self, attr: ir.Attribute) -> ValueType:
        if (method := self.registry.attributes.get(type(attr))) is not None:
            return method(self, attr)
        raise NotImplementedError(f"Attribute {type(attr)} not implemented")

    def emit_stmt_begin(self, frame: FrameType, stmt: ir.Statement) -> None:
        return

    def emit_stmt_end(self, frame: FrameType, stmt: ir.Statement) -> None:
        return

    def emit_block_begin(self, frame: FrameType, block: ir.Block) -> None:
        return

    def emit_block_end(self, frame: FrameType, block: ir.Block) -> None:
        return

    def emit_block(
        self, frame: FrameType, block: ir.Block
    ) -> interp.MethodResult[ValueType]:
        self.emit_block_begin(frame, block)
        stmt = block.first_stmt
        while stmt is not None:
            if self.consume_fuel() == self.FuelResult.Stop:
                raise FuelExhaustedError("fuel exhausted")

            self.emit_stmt_begin(frame, stmt)
            stmt_results = self.run_stmt(frame, stmt)
            self.emit_stmt_end(frame, stmt)

            match stmt_results:
                case interp.Err(_):
                    return stmt_results
                case tuple(values):
                    frame.set_values(stmt._results, values)
                case interp.ReturnValue(_):
                    pass
                case _:
                    raise ValueError(f"Unexpected result {stmt_results}")

            stmt = stmt.next_stmt

        self.emit_block_end(frame, block)
        return frame.block_ref[block]
