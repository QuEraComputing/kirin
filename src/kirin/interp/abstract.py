from abc import ABC
from typing import TypeVar, Iterable
from dataclasses import field, dataclass

from kirin.ir import Region, SSAValue, Statement
from kirin.lattice import BoundedLattice
from kirin.worklist import WorkList
from kirin.interp.base import BaseInterpreter, InterpreterMeta
from kirin.interp.frame import Frame
from kirin.interp.value import Successor, ReturnValue, MethodResult

ResultType = TypeVar("ResultType", bound=BoundedLattice)
WorkListType = TypeVar("WorkListType", bound=WorkList[Successor])


@dataclass
class AbstractFrame(Frame[ResultType]):
    worklist: WorkList[Successor[ResultType]] = field(default_factory=WorkList)


AbstractFrameType = TypeVar("AbstractFrameType", bound=AbstractFrame)

# TODO: support custom loop termination heurestics, e.g. max iteration, etc.
# currently we may end up in infinite loop


class AbstractInterpreterMeta(InterpreterMeta):
    pass


class AbstractInterpreter(
    BaseInterpreter[AbstractFrameType, ResultType],
    ABC,
    metaclass=AbstractInterpreterMeta,
):
    lattice: type[BoundedLattice[ResultType]]
    """lattice type for the abstract interpreter.
    """

    def __init_subclass__(cls) -> None:
        if ABC in cls.__bases__:
            return super().__init_subclass__()

        if not hasattr(cls, "lattice"):
            raise TypeError(
                f"missing lattice attribute in abstract interpreter class {cls}"
            )
        cls.void = cls.lattice.bottom()
        super().__init_subclass__()

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
        self, frame: AbstractFrameType, region: Region
    ) -> MethodResult[ResultType]:
        result = self.void
        frame.worklist.append(
            Successor(region.blocks[0], *frame.get_values(region.blocks[0].args))
        )
        while (succ := frame.worklist.pop()) is not None:
            self.prehook_succ(frame, succ)
            block_result = self.run_block(frame, succ)
            result: ResultType = block_result.join(result)
            self.posthook_succ(frame, succ)
        return result

    def run_block(self, frame: AbstractFrameType, succ: Successor) -> ResultType:
        self.set_values(frame, succ.block.args, succ.block_args)

        stmt = succ.block.first_stmt
        while stmt is not None:
            if self.should_exec_stmt(stmt) is False:
                stmt = stmt.next_stmt  # skip
                continue

            stmt_results = self.run_stmt(frame, stmt)
            match stmt_results:
                case tuple(values):
                    self.set_values(frame, stmt._results, values)
                case ReturnValue(result):  # this must be last stmt in block
                    return result
                case _:  # just ignore other cases
                    pass

            stmt = stmt.next_stmt
        return self.void
