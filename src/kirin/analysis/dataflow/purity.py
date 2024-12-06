# NOTE: replace this with escape analysis maybe in the future

from dataclasses import dataclass
from typing import final

from kirin import ir
from kirin.analysis.dataflow.forward import ForwardExtra
from kirin.interp.base import InterpResult
from kirin.interp.value import Result, ResultValue
from kirin.lattice import Lattice, SingletonMeta


@dataclass
class PurityInfo:
    frame_not_pure: bool = False


class Purity(Lattice["Purity"]):

    @property
    def parent_type(self) -> type["Purity"]:
        return Purity

    @property
    def top(self) -> "Purity":
        return NotPure()

    @property
    def bottom(self) -> "Purity":
        return Pure()

    # TODO: move this to SimpleLattice
    def join(self, other: "Purity") -> "Purity":
        if other.is_subseteq(self):
            return self
        elif self.is_subseteq(other):
            return other
        return Bottom()

    def meet(self, other: "Purity") -> "Purity":
        if other.is_subseteq(self):
            return other
        elif self.is_subseteq(other):
            return self
        return Bottom()


@final
class NotPure(Purity, metaclass=SingletonMeta):

    def is_subseteq(self, other: Purity) -> bool:
        return True

    def __hash__(self):
        return id(self)


@final
class Pure(Purity, metaclass=SingletonMeta):

    def is_subseteq(self, other: Purity) -> bool:
        return isinstance(other, NotPure) or isinstance(other, Pure)

    def __hash__(self):
        return id(self)


@final
class Bottom(Purity, metaclass=SingletonMeta):

    def is_subseteq(self, other: Purity) -> bool:
        return True

    def __hash__(self):
        return id(self)


@final
class PurityAnalysis(ForwardExtra[Purity, PurityInfo]):
    keys = ["purity", "empty"]

    @classmethod
    def bottom_value(cls) -> Purity:
        return Pure()

    def eval_stmt(self, stmt: ir.Statement, args: tuple) -> Result[Purity]:
        if stmt.has_trait(ir.Pure):
            return ResultValue(Pure())

        # custom handling for specific statements
        sig = self.build_signature(stmt, args)
        if sig in self.registry:
            return self.registry[sig](self, stmt, args)
        elif stmt.__class__ in self.registry:
            return self.registry[stmt.__class__](self, stmt, args)

        # NOTE: this will return so result value is not assigned
        # assign manually here.
        frame = self.state.current_frame()
        for result in stmt.results:
            frame.entries[result] = NotPure()
        frame.extra = PurityInfo(frame_not_pure=True)
        return ResultValue(NotPure())

    def run_method_region(
        self, mt: ir.Method, body: ir.Region, args: tuple[Purity, ...]
    ) -> InterpResult[Purity]:
        result = self.run_ssacfg_region(body, (Pure(),) + args)
        extra = self.state.current_frame().extra
        if extra is not None and extra.frame_not_pure:
            return InterpResult(NotPure())
        return result
