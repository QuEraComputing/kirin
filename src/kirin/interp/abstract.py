from abc import abstractmethod
from dataclasses import field
from typing import Generic, Iterable, TypeVar

from kirin.interp.base import BaseInterpreter, InterpResult
from kirin.interp.frame import Frame
from kirin.interp.value import ResultValue, ReturnValue, Successor
from kirin.ir import Dialect, DialectGroup, Region, SSAValue, Statement
from kirin.lattice import Lattice
from kirin.worklist import WorkList

ResultType = TypeVar("ResultType", bound=Lattice)
WorkListType = TypeVar("WorkListType", bound=WorkList[Successor])

# TODO: support custom loop termination heurestics, e.g. max iteration, etc.
# currently we may end up in infinite loop


class AbstractInterpreter(
    Generic[ResultType, WorkListType], BaseInterpreter[ResultType]
):
    bottom: ResultType = field(init=False, kw_only=True, repr=False)
    worklist: WorkListType = field(init=False, kw_only=True, repr=False)

    def __init__(
        self, dialects: DialectGroup | Iterable[Dialect], *, fuel: int | None = None
    ):
        super().__init__(dialects, fuel=fuel)
        self.bottom = self.bottom_value()
        self.worklist = self.default_worklist()

    @classmethod
    @abstractmethod
    def bottom_value(cls) -> ResultType:
        pass

    @classmethod
    @abstractmethod
    def default_worklist(cls) -> WorkListType:
        pass

    def prehook_succ(self, frame: Frame, succ: Successor):
        return

    def posthook_succ(self, frame: Frame, succ: Successor):
        return

    def should_exec_stmt(self, stmt: Statement):
        return True

    def join_results(
        self, values: Iterable[SSAValue], stmt_results: Iterable[ResultType]
    ) -> None:
        return

    def run_ssacfg_region(
        self, region: Region, args: tuple[ResultType, ...]
    ) -> InterpResult[ResultType]:
        frame = self.state.current_frame()
        result = self.bottom
        if not region.blocks:
            return InterpResult(result)

        self.worklist.push(Successor(region.blocks[0], *args))
        while (succ := self.worklist.pop()) is not None:
            self.prehook_succ(frame, succ)
            result = self.run_block(frame, succ).join(result)
            self.posthook_succ(frame, succ)
        return InterpResult(result)

    def run_block(self, frame: Frame, succ: Successor) -> ResultType:
        self.join_results(succ.block.args, succ.block_args)
        frame.set_values(zip(succ.block.args, succ.block_args))

        stmt = succ.block.first_stmt
        while stmt is not None:
            if self.should_exec_stmt(stmt) is False:
                stmt = stmt.next_stmt  # skip
                continue

            inputs = frame.get_values(stmt.args)
            stmt_results = self.run_stmt(stmt, inputs)
            match stmt_results:
                case ResultValue(values):
                    self.join_results(stmt._results, values)
                    frame.set_values(zip(stmt._results, values))
                case ReturnValue(result):  # this must be last stmt in block
                    return result
                case _:  # just ignore other cases
                    pass

            stmt = stmt.next_stmt
        return self.bottom
