from dataclasses import dataclass
from typing import Any, Iterable

from kirin import ir
from kirin.analysis.dataflow.forward import ForwardDataFlowAnalysis
from kirin.exceptions import InterpreterError
from kirin.interp import Interpreter, value as interp_value
from kirin.interp.base import InterpResult
from kirin.lattice import Lattice, LatticeMeta, SingletonMeta
from kirin.worklist import WorkList


@dataclass
class ConstPropLattice(Lattice["ConstPropLattice"]):

    @property
    def parent_type(self) -> type["ConstPropLattice"]:
        return ConstPropLattice

    @classmethod
    def top(cls) -> Any:
        return NotConst()

    @classmethod
    def bottom(cls) -> Any:
        return ConstPropBottom()

    def join(self, other: "ConstPropLattice") -> "ConstPropLattice":
        if other.is_subseteq(self):
            return self
        elif self.is_subseteq(other):
            return other
        return NotConst()

    def meet(self, other: "ConstPropLattice") -> "ConstPropLattice":
        if self.is_subseteq(other):
            return self
        elif other.is_subseteq(self):
            return other
        return ConstPropBottom()


@dataclass
class Const(ConstPropLattice):
    data: Any

    def is_equal(self, other: ConstPropLattice) -> bool:
        if isinstance(other, Const):
            return self.data == other.data
        return False

    def is_subseteq(self, other: ConstPropLattice) -> bool:
        if isinstance(other, Const):
            return self.data == other.data
        elif isinstance(other, NotConst):
            return True
        return False


class PartialTupleMeta(LatticeMeta):
    def __call__(cls, data: tuple[ConstPropLattice, ...]):
        if all(isinstance(x, Const) for x in data):
            return Const(tuple(x.data for x in data))  # type: ignore
        return super().__call__(data)


@dataclass
class PartialTuple(ConstPropLattice, metaclass=PartialTupleMeta):
    data: tuple[ConstPropLattice, ...]

    def join(self, other: ConstPropLattice) -> ConstPropLattice:
        if other.is_subseteq(self):
            return self
        elif self.is_subseteq(other):
            return other
        elif isinstance(other, PartialTuple):
            return PartialTuple(tuple(x.join(y) for x, y in zip(self.data, other.data)))
        elif isinstance(other, Const) and isinstance(other.data, tuple):
            return PartialTuple(
                tuple(x.join(Const(y)) for x, y in zip(self.data, other.data))
            )
        return NotConst()

    def meet(self, other: ConstPropLattice) -> ConstPropLattice:
        if self.is_subseteq(other):
            return self
        elif other.is_subseteq(self):
            return other
        elif isinstance(other, PartialTuple):
            return PartialTuple(tuple(x.meet(y) for x, y in zip(self.data, other.data)))
        elif isinstance(other, Const) and isinstance(other.data, tuple):
            return PartialTuple(
                tuple(x.meet(Const(y)) for x, y in zip(self.data, other.data))
            )
        return ConstPropBottom()

    def is_equal(self, other: ConstPropLattice) -> bool:
        if isinstance(other, PartialTuple):
            return all(x.is_equal(y) for x, y in zip(self.data, other.data))
        elif isinstance(other, Const) and isinstance(other.data, tuple):
            return all(x.is_equal(Const(y)) for x, y in zip(self.data, other.data))
        return False

    def is_subseteq(self, other: ConstPropLattice) -> bool:
        if isinstance(other, PartialTuple):
            return all(x.is_subseteq(y) for x, y in zip(self.data, other.data))
        elif isinstance(other, Const) and isinstance(other.data, tuple):
            return all(x.is_subseteq(Const(y)) for x, y in zip(self.data, other.data))
        elif isinstance(other, NotConst):
            return True
        return False


@dataclass
class ConstPropBottom(ConstPropLattice, metaclass=SingletonMeta):

    def is_equal(self, other: ConstPropLattice) -> bool:
        return self is other

    def is_subseteq(self, other: ConstPropLattice) -> bool:
        return True


@dataclass
class NotConst(ConstPropLattice, metaclass=SingletonMeta):

    def is_equal(self, other: ConstPropLattice) -> bool:
        return self is other

    def is_subseteq(self, other: ConstPropLattice) -> bool:
        return self.is_equal(other)


class ConstProp(
    ForwardDataFlowAnalysis[ConstPropLattice, WorkList[interp_value.Successor]]
):
    keys = ["constprop", "empty"]
    interp: Interpreter

    def __init__(
        self,
        dialects: ir.DialectGroup | Iterable[ir.Dialect],
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
        self.interp = Interpreter(
            dialects,
            fuel=fuel,
            max_depth=max_depth,
            max_python_recursion_depth=max_python_recursion_depth,
        )

    @classmethod
    def bottom_value(cls) -> ConstPropLattice:
        return ConstPropBottom()

    @classmethod
    def default_worklist(cls) -> WorkList[interp_value.Successor]:
        return WorkList()

    def try_eval_const(
        self, stmt: ir.Statement, args: tuple[Const, ...]
    ) -> interp_value.Result[ConstPropLattice]:
        try:
            value = self.interp.eval_stmt(stmt, tuple(x.data for x in args))
            if isinstance(value, interp_value.ResultValue):
                return interp_value.ResultValue(
                    *tuple(Const(each) for each in value.values)
                )
            elif isinstance(value, interp_value.ReturnValue):
                return interp_value.ReturnValue(Const(value.result))
            elif isinstance(value, interp_value.Successor):
                return interp_value.Successor(
                    value.block, *tuple(Const(each) for each in value.block_args)
                )
        except InterpreterError:
            pass
        return interp_value.ResultValue(NotConst())

    def eval_stmt(
        self, stmt: ir.Statement, args: tuple
    ) -> interp_value.Result[ConstPropLattice]:
        frame = self.state.current_frame()
        for result in stmt.results:
            # NOTE: multiple results hit here, terminate early
            if result in frame.entries and isinstance(frame.entries[result], NotConst):
                return interp_value.ResultValue(NotConst())

        if stmt.has_trait(ir.ConstantLike) or (
            stmt.has_trait(ir.Pure) and all(isinstance(x, Const) for x in args)
        ):
            return self.try_eval_const(stmt, args)

        sig = self.build_signature(stmt, args)
        if sig in self.registry:
            return self.registry[sig](self, stmt, args)
        elif stmt.__class__ in self.registry:
            return self.registry[stmt.__class__](self, stmt, args)
        # there is only one fallback
        return interp_value.ResultValue(NotConst())

    def run_method_region(
        self, mt: ir.Method, body: ir.Region, args: tuple[ConstPropLattice, ...]
    ) -> InterpResult[ConstPropLattice]:
        return self.run_ssacfg_region(body, (Const(mt),) + args)
