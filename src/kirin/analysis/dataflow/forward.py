from typing import Generic, Iterable, TypeVar

from kirin.interp import AbstractFrame, AbstractInterpreter
from kirin.ir import Dialect, SSAValue
from kirin.ir.group import DialectGroup
from kirin.ir.method import Method
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

    def postprocess_frame(
        self, frame: ForwardFrame[LatticeElemType, ExtraType]
    ) -> None:
        self.results = frame.entries

    def new_method_frame(self, mt: Method) -> ForwardFrame[LatticeElemType, ExtraType]:
        return ForwardFrame.from_method(mt)


class Forward(ForwardExtra[LatticeElemType, None]):
    pass
