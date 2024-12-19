from typing import TypeVar

from kirin import ir
from kirin.interp.base import FrameABC, BaseInterpreter

FrameType = TypeVar("FrameType", bound=FrameABC)
ValueType = TypeVar("ValueType")


class EmitABC(BaseInterpreter[FrameType, ValueType]):

    def run_ssacfg_region(
        self, region: ir.Region, args: tuple[ValueType, ...]
    ) -> ValueType:
        raise ValueError("run_ssacfg_region should not be called in emit mode")

    # def run_callable(
    #     self, code: ir.Statement, args: tuple[ValueType, ...]
    # ) -> ValueType:
    #     frame = self.new_frame(code)
    #     self.state.push_frame(frame)
    #     results = self.run_stmt(frame, code)
    #     if isinstance(results, SpecialResult):
    #         raise ValueError(f"SpecialResult {results} is not allowed in emit mode")

    #     if results is not None and len(results) != 1:
    #         raise ValueError(f"Expected single result, got {results}")
    #     return self.finalize_results(self.state.pop_frame(), results)
