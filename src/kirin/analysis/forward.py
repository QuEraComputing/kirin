from typing import Generic, TypeVar, Iterable

from kirin import ir
from kirin.interp import MethodResult, AbstractFrame, AbstractInterpreter
from kirin.lattice import BoundedLattice

ExtraType = TypeVar("ExtraType")
LatticeElemType = TypeVar("LatticeElemType", bound=BoundedLattice)


class ForwardFrame(AbstractFrame[LatticeElemType], Generic[LatticeElemType, ExtraType]):
    extra: ExtraType | None = None


class ForwardExtra(
    AbstractInterpreter[ForwardFrame[LatticeElemType, ExtraType], LatticeElemType],
):
    """Abstract interpreter but record results for each SSA value.

    Params:
        LatticeElemType: The lattice element type.
        ExtraType: The type of extra information to be stored in the frame.
    """

    def __init__(
        self,
        dialects: ir.DialectGroup | Iterable[ir.Dialect],
        *,
        fuel: int | None = None,
        save_all_ssa: bool = False,
        max_depth: int = 128,
        max_python_recursion_depth: int = 8192,
    ):
        super().__init__(
            dialects,
            fuel=fuel,
            max_depth=max_depth,
            max_python_recursion_depth=max_python_recursion_depth,
        )
        self.save_all_ssa = save_all_ssa
        self.results: dict[ir.SSAValue, LatticeElemType] = {}

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


class Forward(ForwardExtra[LatticeElemType, None]):
    pass
