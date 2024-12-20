from abc import abstractmethod
from typing import TypeVar
from dataclasses import field, dataclass

from kirin import ir, interp
from kirin.exceptions import FuelExhaustedError

ValueType = TypeVar("ValueType")


@dataclass
class EmitFrame(interp.Frame[ValueType]):
    indent: int = 0
    block_labels: dict[ir.Block, ValueType] = field(default_factory=dict)


FrameType = TypeVar("FrameType", bound=EmitFrame)


class EmitABC(interp.BaseInterpreter[FrameType, ValueType]):

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
            frame.block_labels[block] = block_header

        return result

    @abstractmethod
    def emit_block_header(self, frame: FrameType, block: ir.Block) -> ValueType: ...

    def emit_block(
        self, frame: FrameType, block: ir.Block
    ) -> interp.MethodResult[ValueType]:
        results = self.emit_block_header(frame, block)
        stmt = block.first_stmt
        while stmt is not None:
            if self.consume_fuel() == self.FuelResult.Stop:
                raise FuelExhaustedError("fuel exhausted")

            stmt_results = self.run_stmt(frame, stmt)

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

        return results
