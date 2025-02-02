from abc import ABC
from typing import TypeVar
from dataclasses import field, dataclass

from kirin import ir, interp
from kirin.worklist import WorkList

ValueType = TypeVar("ValueType")


@dataclass
class EmitFrame(interp.Frame[ValueType]):
    worklist: WorkList[interp.Successor] = field(default_factory=WorkList)
    block_ref: dict[ir.Block, ValueType] = field(default_factory=dict)


FrameType = TypeVar("FrameType", bound=EmitFrame)


@dataclass
class EmitABC(interp.BaseInterpreter[FrameType, ValueType], ABC):

    def run_callable_region(
        self, frame: FrameType, code: ir.Statement, region: ir.Region
    ) -> ValueType:
        results = self.eval_stmt(frame, code)
        if isinstance(results, tuple):
            if len(results) == 0:
                return self.void
            elif len(results) == 1:
                return results[0]
        raise interp.InterpreterError(f"Unexpected results {results}")

    def run_ssacfg_region(
        self, frame: FrameType, region: ir.Region
    ) -> tuple[ValueType, ...]:
        frame.worklist.append(
            interp.Successor(region.blocks[0], frame.get_values(region.blocks[0].args))
        )
        while (succ := frame.worklist.pop()) is not None:
            block_header = self.emit_block(frame, succ.block)
            frame.block_ref[succ.block] = block_header
        return ()

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

    def emit_block(self, frame: FrameType, block: ir.Block) -> ValueType:
        self.emit_block_begin(frame, block)
        stmt = block.first_stmt
        while stmt is not None:
            if self.consume_fuel() == self.FuelResult.Stop:
                raise interp.FuelExhaustedError("fuel exhausted")

            self.emit_stmt_begin(frame, stmt)
            stmt_results = self.eval_stmt(frame, stmt)
            self.emit_stmt_end(frame, stmt)

            match stmt_results:
                case tuple(values):
                    frame.set_values(stmt._results, values)
                case interp.ReturnValue(_) | interp.YieldValue(_):
                    pass
                case _:
                    raise ValueError(f"Unexpected result {stmt_results}")

            stmt = stmt.next_stmt

        self.emit_block_end(frame, block)
        return frame.block_ref[block]
