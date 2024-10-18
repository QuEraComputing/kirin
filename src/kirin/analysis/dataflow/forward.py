from abc import abstractmethod
from dataclasses import field
from typing import Iterable, TypeVar

from kirin.interp import AbstractInterpreter, Successor
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
        self, dialects: DialectGroup | Iterable[Dialect], *, fuel: int | None = None
    ):
        super().__init__(dialects, fuel=fuel)
        self.results = {}

    @classmethod
    @abstractmethod
    def bottom_value(cls) -> ResultType:
        pass

    @classmethod
    @abstractmethod
    def default_worklist(cls) -> WorkListType:
        pass

    def join_results(
        self, values: Iterable[SSAValue], stmt_results: Iterable[ResultType]
    ) -> None:
        for result, value in zip(values, stmt_results):
            if result in self.results:
                self.results[result] = self.results[result].join(value)
            else:
                self.results[result] = value
