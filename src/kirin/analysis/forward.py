from abc import ABC
from typing import Generic, TypeVar, Iterable
from dataclasses import field, dataclass

from kirin import ir, interp
from kirin.interp import AbstractFrame, AbstractInterpreter
from kirin.lattice import BoundedLattice

ExtraType = TypeVar("ExtraType")
LatticeElemType = TypeVar("LatticeElemType", bound=BoundedLattice)


class ForwardFrame(AbstractFrame[LatticeElemType], Generic[LatticeElemType, ExtraType]):
    extra: ExtraType | None = None


@dataclass
class ForwardExtra(
    AbstractInterpreter[ForwardFrame[LatticeElemType, ExtraType], LatticeElemType],
    ABC,
):
    """Abstract interpreter but record results for each SSA value.

    Params:
        LatticeElemType: The lattice element type.
        ExtraType: The type of extra information to be stored in the frame.
    """

    save_all_ssa: bool = field(default=False, kw_only=True)
    results: dict[ir.SSAValue, LatticeElemType] = field(
        default_factory=dict, init=False, compare=False
    )

    def initialize(self):
        super().initialize()
        self.results: dict[ir.SSAValue, LatticeElemType] = {}
        return self

    def run_analysis(
        self,
        method: ir.Method,
        args: tuple[LatticeElemType, ...] | None = None,
    ) -> tuple[dict[ir.SSAValue, LatticeElemType], LatticeElemType]:
        """Run the forward dataflow analysis.

        Args:
            method(ir.Method): The method to analyze.
            args(tuple[LatticeElemType]): The arguments to the method. Defaults to tuple of top values.

        Keyword Args:
            save_all_ssa(bool): If True, save all SSA values in the results.

        Returns:
            dict[ir.SSAValue, LatticeElemType]: The results of the analysis for each SSA value.
            LatticeElemType: The result of the analysis for the method return value.
        """
        args = args or tuple(self.lattice.top() for _ in method.args)
        result = self.run(method, args)
        if isinstance(result, interp.result.Err):
            return self.results, self.lattice.bottom()
        return self.results, result.expect()

    def set_values(
        self,
        frame: AbstractFrame[LatticeElemType],
        ssa: Iterable[ir.SSAValue],
        results: Iterable[LatticeElemType],
    ):
        for ssa_value, result in zip(ssa, results):
            if ssa_value in frame.entries:
                frame.entries[ssa_value] = frame.entries[ssa_value].join(result)
            else:
                frame.entries[ssa_value] = result

    def finalize(
        self,
        frame: ForwardFrame[LatticeElemType, ExtraType],
        results: LatticeElemType,
    ) -> LatticeElemType:
        if self.save_all_ssa:
            self.results.update(frame.entries)
        else:
            self.results = frame.entries
        return results

    def new_frame(self, code: ir.Statement) -> ForwardFrame[LatticeElemType, ExtraType]:
        return ForwardFrame.from_func_like(code)


class Forward(ForwardExtra[LatticeElemType, None], ABC):
    pass
