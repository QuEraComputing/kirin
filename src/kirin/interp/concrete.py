from typing import Any

from kirin.ir import Block, Region
from kirin.ir.method import Method
from kirin.ir.nodes.stmt import Statement

from .base import BaseInterpreter
from .frame import Frame
from .value import Successor, ReturnValue
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

    def run_method(self, method: Method, args: tuple[Any, ...]) -> Any:
        return self.run_callable(method.code, (method,) + args)

    def run_ssacfg_region(self, frame: Frame[Any], region: Region) -> Any:
        stmt_idx = 0
        result = self.void
        block: Block | None = region.blocks[0]
        while block is not None:
            stmt = block.first_stmt

            # TODO: make this more precise
            frame.stmt = stmt
            frame.lino = stmt_idx
            block = None

            while stmt is not None:
                if self.consume_fuel() == self.FuelResult.Stop:
                    raise FuelExhaustedError("fuel exhausted")

                # TODO: make this more precise
                frame.lino = stmt_idx
                frame.stmt = stmt
                stmt_results = self.eval_stmt(frame, stmt)

                match stmt_results:
                    case tuple(values):
                        frame.set_values(stmt._results, values)
                    case ReturnValue(result):
                        break
                    case Successor(block, block_args):
                        # block is not None, continue to next block
                        frame.set_values(block.args, block_args)
                        break
                    case _:
                        pass

                stmt = stmt.next_stmt
                stmt_idx += 1
        # end of while
        return result
