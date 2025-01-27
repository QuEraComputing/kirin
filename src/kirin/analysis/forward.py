from abc import ABC
from typing import Generic, TypeVar, Iterable

from kirin import ir, interp
from kirin.interp import MethodResult, AbstractFrame, AbstractInterpreter
from kirin.lattice import BoundedLattice

ExtraType = TypeVar("ExtraType")
LatticeElemType = TypeVar("LatticeElemType", bound=BoundedLattice)


class ForwardFrame(AbstractFrame[LatticeElemType], Generic[LatticeElemType, ExtraType]):
    extra: ExtraType | None = None


class ForwardExtra(
    AbstractInterpreter[ForwardFrame[LatticeElemType, ExtraType], LatticeElemType],
    ABC,
):
    """Abstract interpreter but record results for each SSA value.

    Params:
        LatticeElemType: The lattice element type.
        ExtraType: The type of extra information to be stored in the frame.
    """

    def initialize(self, save_all_ssa: bool = False, *args, **kwargs):
        super().initialize()
        self.save_all_ssa = save_all_ssa
        self.results: dict[ir.SSAValue, LatticeElemType] = {}

    def run(
        self, method: ir.Method, *, save_all_ssa: bool = False, **kwargs
    ) -> tuple[dict[ir.SSAValue, LatticeElemType], LatticeElemType]:
        """Run the forward dataflow analysis.

        Args:
            method(ir.Method): The method to analyze.

        Keyword Args:
            save_all_ssa(bool): If True, save all SSA values in the results.

        Returns:
            dict[ir.SSAValue, LatticeElemType]: The results of the analysis for each SSA value.
            LatticeElemType: The result of the analysis for the method return value.
        """
        self.initialize(save_all_ssa=save_all_ssa)
        result = self.eval(method, tuple(self.lattice.top() for _ in method.args))
        if isinstance(result.value, interp.Err):
            return self.results, self.lattice.bottom()
        return self.results, result.value

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
        results: MethodResult[LatticeElemType],
    ) -> MethodResult[LatticeElemType]:
        if self.save_all_ssa:
            self.results.update(frame.entries)
        else:
            self.results = frame.entries
        return results

    def new_frame(self, code: ir.Statement) -> ForwardFrame[LatticeElemType, ExtraType]:
        return ForwardFrame.from_func_like(code)


class Forward(ForwardExtra[LatticeElemType, None], ABC):
    pass
