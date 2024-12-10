from abc import abstractmethod
from dataclasses import dataclass, field
from typing import Iterable, TypeVar

from kirin.interp.base import BaseInterpreter, InterpResult
from kirin.interp.frame import Frame
from kirin.interp.value import ResultValue, ReturnValue, Successor
from kirin.ir import Dialect, DialectGroup, Region, SSAValue, Statement
from kirin.ir.method import Method
from kirin.lattice import Lattice
from kirin.worklist import WorkList

ResultType = TypeVar("ResultType", bound=Lattice)
WorkListType = TypeVar("WorkListType", bound=WorkList[Successor])


@dataclass
class AbstractFrame(Frame[ResultType]):
    worklist: WorkList[Successor[ResultType]] = field(default_factory=WorkList)


AbstractFrameType = TypeVar("AbstractFrameType", bound=AbstractFrame)

# TODO: support custom loop termination heurestics, e.g. max iteration, etc.
# currently we may end up in infinite loop


class AbstractInterpreter(
    BaseInterpreter[AbstractFrameType, ResultType],
):
    lattice: type[Lattice]
    """lattice type for the abstract interpreter.
    """

    def __init__(
        self,
        dialects: DialectGroup | Iterable[Dialect],
        *,
        fuel: int | None = None,
        max_depth: int = 128,
        max_python_recursion_depth: int = 8192,
    ):
        if not hasattr(self, "lattice"):
            raise TypeError(f"lattice is not defined for {self.__class__.__name__}")
        super().__init__(
            dialects,
            fuel=fuel,
            max_depth=max_depth,
            max_python_recursion_depth=max_python_recursion_depth,
        )
        self.bottom: ResultType = self.lattice.bottom()

    @abstractmethod
    def new_method_frame(self, mt: Method) -> AbstractFrameType: ...

    def prehook_succ(self, frame: AbstractFrameType, succ: Successor):
        return

    def posthook_succ(self, frame: AbstractFrameType, succ: Successor):
        return

    def should_exec_stmt(self, stmt: Statement):
        return True

    def set_values(
        self,
        frame: AbstractFrameType,
        ssa: Iterable[SSAValue],
        results: Iterable[ResultType],
    ):
        frame.set_values(ssa, results)

    def run_ssacfg_region(
        self, region: Region, args: tuple[ResultType, ...]
    ) -> InterpResult[ResultType]:
        frame = self.state.current_frame()
        result = self.bottom
        if not region.blocks:
            return InterpResult(result)

        frame.worklist.append(Successor(region.blocks[0], *args))
        while (succ := frame.worklist.pop()) is not None:
            self.prehook_succ(frame, succ)
            result = self.run_block(frame, succ).join(result)
            self.posthook_succ(frame, succ)
        return InterpResult(result)

    def run_block(self, frame: AbstractFrameType, succ: Successor) -> ResultType:
        self.set_values(frame, succ.block.args, succ.block_args)

        stmt = succ.block.first_stmt
        while stmt is not None:
            if self.should_exec_stmt(stmt) is False:
                stmt = stmt.next_stmt  # skip
                continue

            inputs = frame.get_values(stmt.args)
            stmt_results = self.run_stmt(stmt, inputs)
            match stmt_results:
                case ResultValue(values):
                    self.set_values(frame, stmt._results, values)
                case ReturnValue(result):  # this must be last stmt in block
                    return result
                case _:  # just ignore other cases
                    pass

            stmt = stmt.next_stmt
        return self.bottom
