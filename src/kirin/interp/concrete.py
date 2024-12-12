from typing import Any, Iterable

from kirin.ir import Block, Region, Dialect, DialectGroup
from kirin.ir.method import Method
from kirin.exceptions import FuelExhaustedError
from kirin.interp.base import BaseInterpreter
from kirin.interp.frame import Frame
from kirin.interp.value import Err, NoReturn, Successor, ReturnValue


class Interpreter(BaseInterpreter[Frame[Any], Any]):
    keys = ["main", "empty"]

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
            fuel=fuel,
            max_depth=max_depth,
            max_python_recursion_depth=max_python_recursion_depth,
        )

    def new_method_frame(self, mt: Method) -> Frame[Any]:
        return Frame.from_method(mt)

    def run_method_region(self, mt: Method, body: Region, args: tuple[Any, ...]) -> Any:
        return self.run_ssacfg_region(body, (mt,) + args)

    def run_ssacfg_region(self, region: Region, args: tuple[Any, ...]) -> Any:
        result: Any = NoReturn()
        frame = self.state.current_frame()
        # empty body, return
        if not region.blocks:
            return result

        stmt_idx = 0
        block: Block | None = region.blocks[0]
        while block is not None:
            frame.set_values(block.args, args)
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
                        return stmt_results
                    case tuple(values):
                        frame.set_values(stmt._results, values)
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
        return result
