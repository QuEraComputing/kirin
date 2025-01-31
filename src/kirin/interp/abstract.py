from abc import ABC
from typing import TypeVar, Iterable
from dataclasses import field, dataclass

from kirin.ir import Region, SSAValue, Statement
from kirin.lattice import BoundedLattice
from kirin.worklist import WorkList
from kirin.interp.base import BaseInterpreter, InterpreterMeta
from kirin.interp.frame import Frame
from kirin.interp.value import Successor, ReturnValue

ResultType = TypeVar("ResultType", bound=BoundedLattice)
WorkListType = TypeVar("WorkListType", bound=WorkList[Successor])


@dataclass
class AbstractFrame(Frame[ResultType]):
    """Interpreter frame for abstract interpreter.

    This frame is used to store the state of the abstract interpreter.
    It contains the worklist of successors to be processed.
    """

    worklist: WorkList[Successor[ResultType]] = field(default_factory=WorkList)


AbstractFrameType = TypeVar("AbstractFrameType", bound=AbstractFrame)

# TODO: support custom loop termination heurestics, e.g. max iteration, etc.
# currently we may end up in infinite loop


class AbstractInterpreterMeta(InterpreterMeta):
    pass


@dataclass
class AbstractInterpreter(
    BaseInterpreter[AbstractFrameType, ResultType],
    ABC,
    metaclass=AbstractInterpreterMeta,
):
    """Abstract interpreter for the IR.

    This is a base class for implementing abstract interpreters for the IR.
    It provides a framework for implementing abstract interpreters given a
    bounded lattice type.

    The abstract interpreter is a forward dataflow analysis that computes
    the abstract values for each SSA value in the IR. The abstract values
    are computed by evaluating the statements in the IR using the abstract
    lattice operations.

    The abstract interpreter is implemented as a worklist algorithm. The
    worklist contains the successors of the current block to be processed.
    The abstract interpreter processes each successor by evaluating the
    statements in the block and updating the abstract values in the frame.

    The abstract interpreter provides hooks for customizing the behavior of
    the interpreter.
    The [`prehook_succ`][kirin.interp.abstract.AbstractInterpreter.prehook_succ] and
    [`posthook_succ`][kirin.interp.abstract.AbstractInterpreter.posthook_succ] methods
    can be used to perform custom actions before and after processing a successor.
    """

    lattice: type[BoundedLattice[ResultType]] = field(init=False)
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
        """Hook called before processing a successor.

        This method can be used to perform custom actions before processing
        a successor. It is called before evaluating the statements in the block.

        Args:
            frame: The current frame of the interpreter.
            succ: The successor to be processed.
        """
        return

    def posthook_succ(self, frame: AbstractFrameType, succ: Successor):
        """Hook called after processing a successor.

        This method can be used to perform custom actions after processing
        a successor. It is called after evaluating the statements in the block.

        Args:
            frame: The current frame of the interpreter.
            succ: The successor that was processed.
        """
        return

    def should_exec_stmt(self, stmt: Statement) -> bool:
        """This method can be used to control which statements are executed
        during the abstract interpretation. By default, all statements are
        executed.

        This method is useful when one wants to skip certain statements
        during the abstract interpretation and is certain that the skipped
        statements do not affect the final result. This would allow saving
        computation time and memory by not evaluating the skipped statements
        and their results.

        Args:
            stmt: The statement to be executed.

        Returns:
            True if the statement should be executed, False otherwise.
        """
        return True

    def set_values(
        self,
        frame: AbstractFrameType,
        ssa: Iterable[SSAValue],
        results: Iterable[ResultType],
    ):
        """Set the abstract values for the given SSA values in the frame.

        This method is used to customize how the abstract values are set in
        the frame. By default, the abstract values are set directly in the
        frame.
        """
        frame.set_values(ssa, results)

    def eval_recursion_limit(self, frame: AbstractFrameType) -> ResultType:
        return self.lattice.bottom()

    def run_ssacfg_region(
        self, frame: AbstractFrameType, region: Region
    ) -> tuple[ResultType, ...]:
        result: tuple[ResultType, ...] = ()
        frame.worklist.append(
            Successor(region.blocks[0], *frame.get_values(region.blocks[0].args))
        )
        while (succ := frame.worklist.pop()) is not None:
            self.prehook_succ(frame, succ)
            block_result = self.run_block(frame, succ)
            result = (
                tuple(old.join(new) for old, new in zip(result, block_result))
                if result
                else block_result
            )
            self.posthook_succ(frame, succ)
        return result

    def run_block(
        self, frame: AbstractFrameType, succ: Successor
    ) -> tuple[ResultType, ...]:
        self.set_values(frame, succ.block.args, succ.block_args)

        stmt = succ.block.first_stmt
        while stmt is not None:
            if self.should_exec_stmt(stmt) is False:
                stmt = stmt.next_stmt  # skip
                continue

            stmt_results = self.eval_stmt(frame, stmt)
            match stmt_results:
                case tuple(values):
                    self.set_values(frame, stmt._results, values)
                case ReturnValue(results):  # this must be last stmt in block
                    return results
                case _:  # just ignore other cases
                    pass

            stmt = stmt.next_stmt
        return ()
