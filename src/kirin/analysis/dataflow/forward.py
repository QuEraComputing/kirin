from abc import abstractmethod
from dataclasses import field
from typing import Iterable, TypeVar

from kirin.interp import AbstractInterpreter, Successor
from kirin.interp.frame import Frame
from kirin.ir import Dialect, SSAValue
from kirin.ir.group import DialectGroup
from kirin.lattice import Lattice
from kirin.worklist import WorkList

ResultType = TypeVar("ResultType", bound=Lattice)
WorkListType = TypeVar("WorkListType", bound=WorkList[Successor])


class ForwardDataFlowAnalysis(AbstractInterpreter[ResultType, WorkListType]):
    """Abstract interpreter but record results for each SSA value."""

    results: dict[SSAValue, ResultType] = field(init=False, default_factory=dict)

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
        self.results = {}

    @classmethod
    @abstractmethod
    def bottom_value(cls) -> ResultType:
        pass

    @classmethod
    @abstractmethod
    def default_worklist(cls) -> WorkListType:
        pass

    def set_values(
        self,
        frame: Frame[ResultType],
        ssa: Iterable[SSAValue],
        results: Iterable[ResultType],
    ):
        for ssa_value, result in zip(ssa, results):
            if ssa_value in frame.entries:
                frame.entries[ssa_value] = frame.entries[ssa_value].join(result)
            else:
                frame.entries[ssa_value] = result

    def postprocess_frame(self, frame: Frame[ResultType]) -> None:
        self.results = frame.entries
