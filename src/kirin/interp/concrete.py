from dataclasses import dataclass
from typing import Any

from kirin.exceptions import FuelExhaustedError
from kirin.interp.base import BaseInterpreter, InterpResult
from kirin.interp.value import Err, NoReturn, ResultValue, ReturnValue, Successor
from kirin.ir import Block, Region


@dataclass(init=False)
class Interpreter(BaseInterpreter[Any]):
    keys = ["main", "empty"]

    def run_ssacfg_region(
        self, region: Region, args: tuple[Any, ...]
    ) -> InterpResult[Any]:
        result: Any = NoReturn()
        frame = self.state.current_frame()
        # empty body, return
        if not region.blocks:
            return InterpResult(result)

        stmt_idx = 0
        block: Block | None = region.blocks[0]
        while block is not None:
            frame.set_values(zip(block.args, args))
            stmt = block.first_stmt

            # TODO: make this more precise
            frame.stmt = stmt
            frame.lino = stmt_idx
            block = None

            while stmt is not None:
                if self.consume_fuel() == self.FuelResult.Stop:
                    raise FuelExhaustedError("fuel exhausted")

                inputs = frame.get_values(stmt.args)
                # TODO: make this more precise
                frame.lino = stmt_idx
                frame.stmt = stmt
                stmt_results = self.run_stmt(stmt, inputs)

                match stmt_results:
                    case Err(_):
                        return InterpResult(stmt_results)
                    case ResultValue(values):
                        frame.set_values(zip(stmt._results, values))
                    case ReturnValue(result):
                        break
                    case Successor(block, block_args):
                        # block is not None, continue to next block
                        args = block_args
                        break
                    case _:
                        pass

                stmt = stmt.next_stmt
                stmt_idx += 1
        # end of while
        return InterpResult(result)
