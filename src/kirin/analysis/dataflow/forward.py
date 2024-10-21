from dataclasses import field
from typing import Iterable, TypeVar

from kirin.interp import AbstractFrame, AbstractInterpreter
from kirin.ir import Dialect, SSAValue
from kirin.ir.group import DialectGroup
from kirin.ir.method import Method
from kirin.lattice import Lattice

LatticeElemType = TypeVar("LatticeElemType", bound=Lattice)


class ForwardDataFlowAnalysis(
    AbstractInterpreter[AbstractFrame[LatticeElemType], LatticeElemType]
):
    """Abstract interpreter but record results for each SSA value."""

    results: dict[SSAValue, LatticeElemType] = field(init=False, default_factory=dict)

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

    def set_values(
        self,
        frame: AbstractFrame[LatticeElemType],
        ssa: Iterable[SSAValue],
        results: Iterable[LatticeElemType],
    ):
        for ssa_value, result in zip(ssa, results):
            if ssa_value in frame.entries:
                frame.entries[ssa_value] = frame.entries[ssa_value].join(result)
            else:
                frame.entries[ssa_value] = result

    def postprocess_frame(self, frame: AbstractFrame[LatticeElemType]) -> None:
        self.results = frame.entries

    def new_method_frame(self, mt: Method) -> AbstractFrame[LatticeElemType]:
        return AbstractFrame.from_method(mt)
