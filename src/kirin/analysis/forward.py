import sys
from abc import ABC
from typing import TypeVar, Iterable
from dataclasses import dataclass

from kirin import ir, interp
from kirin.interp import AbstractFrame, AbstractInterpreter
from kirin.lattice import BoundedLattice

ExtraType = TypeVar("ExtraType")
LatticeElemType = TypeVar("LatticeElemType", bound=BoundedLattice)


@dataclass
class ForwardFrame(AbstractFrame[LatticeElemType]):
    pass


ForwardFrameType = TypeVar("ForwardFrameType", bound=ForwardFrame)


@dataclass
class ForwardExtra(
    AbstractInterpreter[ForwardFrameType, LatticeElemType],
    ABC,
):
    """Abstract interpreter but record results for each SSA value.

    Params:
        LatticeElemType: The lattice element type.
        ExtraType: The type of extra information to be stored in the frame.
    """

    def run_analysis(
        self,
        method: ir.Method,
        args: tuple[LatticeElemType, ...] | None = None,
        *,
        no_raise: bool = True,
    ) -> tuple[ForwardFrameType, LatticeElemType]:
        """Run the forward dataflow analysis.

        Args:
            method(ir.Method): The method to analyze.
            args(tuple[LatticeElemType]): The arguments to the method. Defaults to tuple of top values.

        Keyword Args:
            no_raise(bool): If True, return bottom values if the analysis fails. Defaults to True.

        Returns:
            ForwardFrameType: The results of the analysis contained in the frame.
            LatticeElemType: The result of the analysis for the method return value.
        """
        args = args or tuple(self.lattice.top() for _ in method.args)

        if self._eval_lock:
            raise interp.InterpreterError(
                "recursive eval is not allowed, use run_method instead"
            )

        self._eval_lock = True
        self.initialize()
        current_recursion_limit = sys.getrecursionlimit()
        sys.setrecursionlimit(self.max_python_recursion_depth)
        try:
            frame, ret = self.run_method(method, args)
        except interp.InterpreterError as e:
            # NOTE: initialize will create new State
            # so we don't need to copy the frames.
            if not no_raise:
                raise e
            return self.new_frame(method.code), self.lattice.bottom()
        finally:
            self._eval_lock = False
            sys.setrecursionlimit(current_recursion_limit)
        return frame, ret

    def set_values(
        self,
        frame: AbstractFrame[LatticeElemType],
        ssa: Iterable[ir.SSAValue],
        results: Iterable[LatticeElemType],
    ):
        """Set the abstract values for the given SSA values in the frame.

        This method is used to customize how the abstract values are set in
        the frame. By default, the abstract values are set directly in the
        frame. This method is overridden to join the results if the SSA value
        already exists in the frame.
        """
        for ssa_value, result in zip(ssa, results):
            if ssa_value in frame.entries:
                frame.entries[ssa_value] = frame.entries[ssa_value].join(result)
            else:
                frame.entries[ssa_value] = result


class Forward(ForwardExtra[ForwardFrame[LatticeElemType], LatticeElemType], ABC):
    """Forward dataflow analysis.

    This is the base class for forward dataflow analysis. If your analysis
    requires extra information per frame, you should subclass
    [`ForwardExtra`][kirin.analysis.forward.ForwardExtra] instead.
    """

    def new_frame(self, code: ir.Statement) -> ForwardFrame[LatticeElemType]:
        return ForwardFrame.from_func_like(code)
