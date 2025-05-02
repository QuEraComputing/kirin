from __future__ import annotations

from abc import ABC, abstractmethod
from typing import TypeVar, Iterable
from dataclasses import dataclass

from kirin import ir, interp, lattice

LatticeType = TypeVar("LatticeType", bound=lattice.BoundedLattice)


@dataclass
class ForwardFrame(interp.AbstractFrame[LatticeType]):

    def set_values(
        self, keys: Iterable[ir.SSAValue], values: Iterable[LatticeType]
    ) -> None:
        for ssa_value, result in zip(keys, values):
            if ssa_value in self.entries:
                self.entries[ssa_value] = self.entries[ssa_value].join(result)
            else:
                self.entries[ssa_value] = result


FrameType = TypeVar("FrameType", bound=ForwardFrame)


@dataclass
class ForwardExtra(interp.AbstractInterpreter[FrameType, LatticeType], ABC):
    """Forward dataflow analysis but with custom frame for extra information/state
    per call frame.

    Params:
        FrameType: The type of the frame used for the analysis.
        LatticeType: The type of the lattice used for the analysis.
    """

    def run(self, method: ir.Method, *args: LatticeType, **kwargs: LatticeType):
        if not args and not kwargs:  # empty args and kwargs
            args = tuple(self.lattice.top() for _ in method.args)
        return self.call(method, self.method_self(method), *args, **kwargs)

    def run_no_raise(
        self, method: ir.Method, *args: LatticeType, **kwargs: LatticeType
    ):
        try:
            return self.run(method, *args, **kwargs)
        except Exception:
            return self.initialize_frame(method.code), self.lattice.bottom()

    @abstractmethod
    def method_self(self, method: ir.Method) -> LatticeType:
        """Return the self value for the given method."""
        ...


@dataclass
class Forward(ForwardExtra[ForwardFrame[LatticeType], LatticeType], ABC):
    """Forward dataflow analysis.

    This is the base class for forward dataflow analysis. If your analysis
    requires extra information per frame, you should subclass
    [`ForwardExtra`][kirin.analysis.forward.ForwardExtra] instead.
    """

    def initialize_frame(
        self, node: ir.Statement, *, has_parent_access: bool = False
    ) -> ForwardFrame[LatticeType]:
        return ForwardFrame(node, has_parent_access=has_parent_access)
