from typing import Any

from kirin.ir import Region
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

    def initialize_frame(
        self, code: Statement, *, has_parent_access: bool = False
    ) -> Frame[Any]:
        return Frame(code, has_parent_access=has_parent_access)

    def run_method(
        self, method: Method, args: tuple[Any, ...]
    ) -> tuple[Frame[Any], Any]:
        return self.run_callable(method.code, (method,) + args)

    def run_ssacfg_region(
        self, frame: Frame[Any], region: Region, args: tuple[Any, ...]
    ) -> tuple[Any, ...] | None | ReturnValue[Any]:
        block = region.blocks[0]
        succ = Successor(block, *args)
        while succ is not None:
            results = self.run_succ(frame, succ)
            if isinstance(results, Successor):
                succ = results
            elif isinstance(results, ReturnValue):
                return results
            elif isinstance(results, YieldValue):
                return results.values
            else:
                return results
        return None  # region without terminator returns empty tuple

    def run_succ(self, frame: Frame[Any], succ: Successor) -> SpecialValue[Any]:
        frame.current_block = succ.block
        frame.set_values(succ.block.args, succ.block_args)
        for stmt in succ.block.stmts:
            if self.consume_fuel() == self.FuelResult.Stop:
                raise FuelExhaustedError("fuel exhausted")
            frame.current_stmt = stmt
            stmt_results = self.eval_stmt(frame, stmt)
            if isinstance(stmt_results, tuple):
                frame.set_values(stmt._results, stmt_results)
            elif stmt_results is None:
                continue  # empty result
            else:  # terminator
                return stmt_results
        return None
