from typing import Any, Iterable

from kirin.ir import Block, Region, Dialect, DialectGroup
from kirin.ir.method import Method
from kirin.exceptions import InterpreterError, FuelExhaustedError
from kirin.interp.base import BaseInterpreter
from kirin.interp.frame import Frame
from kirin.interp.value import Err, Successor, ReturnValue, MethodResult
from kirin.ir.nodes.stmt import Statement


class Interpreter(BaseInterpreter[Frame[Any], Any]):
    keys = ["main"]

    def __init__(
        self,
        dialects: DialectGroup | Iterable[Dialect],
        *,
        fuel: int | None = None,
        max_depth: int = 128,
        max_python_recursion_depth: int = 8192,
    ):
        super().__init__(
            dialects,
            bottom=None,
            fuel=fuel,
            max_depth=max_depth,
            max_python_recursion_depth=max_python_recursion_depth,
        )

    def new_frame(self, code: Statement) -> Frame[Any]:
        return Frame.from_func_like(code)

    def run_method(self, method: Method, args: tuple[Any, ...]) -> MethodResult[Any]:
        if len(self.state.frames) >= self.max_depth:
            raise InterpreterError("maximum recursion depth exceeded")
        return self.run_callable(method.code, (method,) + args)

    def run_ssacfg_region(self, frame: Frame[Any], region: Region) -> MethodResult[Any]:
        stmt_idx = 0
        result = self.bottom
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
                stmt_results = self.run_stmt(frame, stmt)

                match stmt_results:
                    case Err(_):
                        return stmt_results
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
