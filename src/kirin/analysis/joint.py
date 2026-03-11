from dataclasses import dataclass

from kirin import ir, interp
from kirin.lattice import BoundedLattice

from .forward import Forward, ForwardFrame


@dataclass
class JointLattice(BoundedLattice["JointLattice"]):
    sublattices: tuple[BoundedLattice, ...]

    @classmethod
    def top(cls) -> "JointLattice":
        return cls(tuple(sublattice.top() for sublattice in cls.sublattices))

    @classmethod
    def bottom(cls) -> "JointLattice":
        return cls(tuple(sublattice.bottom() for sublattice in cls.sublattices))

    def is_subseteq(self, other: "JointLattice") -> bool:
        return all(
            sublattice.is_subseteq(other_sublattice)
            for sublattice, other_sublattice in zip(self.sublattices, other.sublattices)
        )

    def join(self, other: "JointLattice") -> "JointLattice":
        return JointLattice(
            tuple(
                sublattice.join(other_sublattice)
                for sublattice, other_sublattice in zip(
                    self.sublattices, other.sublattices
                )
            )
        )

    def meet(self, other: "JointLattice") -> "JointLattice":
        return JointLattice(
            tuple(
                sublattice.meet(other_sublattice)
                for sublattice, other_sublattice in zip(
                    self.sublattices, other.sublattices
                )
            )
        )


@dataclass
class JointAnalysis(Forward[JointLattice]):
    keys = ["constprop"]
    lattice = JointLattice
    subanalyses: tuple[Forward, ...]

    def initialize(self):
        super().initialize()
        for analysis in self.subanalyses:
            analysis.initialize()
        return self

    def eval_stmt(
        self, frame: ForwardFrame[JointLattice, None], stmt: ir.Statement
    ) -> interp.StatementResult[JointLattice]:
        results = tuple(
            analysis.eval_stmt(analysis.state.current_frame(), stmt)
            for analysis in self.subanalyses
        )
        first_result = results[0]
        if isinstance(first_result, tuple):
            tuple(JointLattice(each_results) for each_results in zip(*results))
