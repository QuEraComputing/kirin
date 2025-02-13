from typing import Any

from kirin.ir import Block, Region
from kirin.ir.method import Method
from kirin.ir.nodes.stmt import Statement

from .base import BaseInterpreter
from .frame import Frame
from .value import Successor, YieldValue, ReturnValue, SpecialValue
from .exceptions import FuelExhaustedError


class Interpreter(BaseInterpreter[Frame[Any], Any]):
    """Concrete interpreter for the IR.

    This is a concrete interpreter for the IR. It evaluates the IR by
    executing the statements in the IR using a simple stack-based
    interpreter.
    """

    keys = ["main"]
    void = None

    def new_frame(self, code: Statement) -> Frame[Any]:
        return Frame.from_func_like(code)

    def run_method(
        self, method: Method, args: tuple[Any, ...]
    ) -> tuple[Frame[Any], Any]:
        return self.run_callable(method.code, (method,) + args)

    def run_ssacfg_region(
        self, frame: Frame[Any], region: Region
    ) -> tuple[Any, ...] | None | ReturnValue[Any]:
        block = region.blocks[0]
        while block is not None:
            results = self.run_block(frame, block)
            if isinstance(results, Successor):
                block = results.block
                frame.set_values(block.args, results.block_args)
            elif isinstance(results, ReturnValue):
                return results
            elif isinstance(results, YieldValue):
                return results.values
            else:
                return results
        return None  # region without terminator returns empty tuple

    def run_block(self, frame: Frame[Any], block: Block) -> SpecialValue[Any]:
        for stmt in block.stmts:
            if self.consume_fuel() == self.FuelResult.Stop:
                raise FuelExhaustedError("fuel exhausted")
            frame.stmt = stmt
            frame.lino = stmt.source.lineno if stmt.source else 0
            stmt_results = self.eval_stmt(frame, stmt)
            if isinstance(stmt_results, tuple):
                frame.set_values(stmt._results, stmt_results)
            elif stmt_results is None:
                continue  # empty result
            else:  # terminator
                return stmt_results
        return None
