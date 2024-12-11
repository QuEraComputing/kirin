from dataclasses import dataclass
from typing import Iterable

from kirin import exceptions, interp, ir
from kirin.analysis import const
from kirin.analysis.forward import ForwardExtra
from kirin.analysis.purity import NotPure, Pure, Purity
from kirin.analysis.typeinfer import TypeInference
from kirin.ir.types import TypeAttribute
from kirin.lattice import BoundedLattice


@dataclass
class JointResult(BoundedLattice["JointResult"]):
    typ: TypeAttribute
    const: const.Result
    purity: Purity

    @classmethod
    def top(cls) -> "JointResult":
        return cls(TypeAttribute.top(), const.Result.top(), Purity.top())

    @classmethod
    def bottom(cls) -> "JointResult":
        return cls(TypeAttribute.bottom(), const.Result.bottom(), Purity.bottom())

    def is_subseteq(self, other: "JointResult") -> bool:
        return (
            self.typ.is_subseteq(other.typ)
            and self.const.is_subseteq(other.const)
            and self.purity.is_subseteq(other.purity)
        )

    def join(self, other: "JointResult") -> "JointResult":
        return JointResult(
            self.typ.join(other.typ),
            self.const.join(other.const),
            self.purity.join(other.purity),
        )

    def meet(self, other: "JointResult") -> "JointResult":
        return JointResult(
            self.typ.meet(other.typ),
            self.const.meet(other.const),
            self.purity.meet(other.purity),
        )


@dataclass
class ExtraFrameInfo:
    frame_not_pure: bool = False


@dataclass
class JointInference(ForwardExtra[JointResult, ExtraFrameInfo]):
    keys = ["inference", "empty"]
    lattice = JointResult

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
        self.constprop = const.Propagate(
            dialects,
            fuel=fuel,
            max_depth=max_depth,
            max_python_recursion_depth=max_python_recursion_depth,
        )
        self.typeinfer = TypeInference(
            dialects,
            fuel=fuel,
            max_depth=max_depth,
            max_python_recursion_depth=max_python_recursion_depth,
        )

    def eval_stmt(
        self, stmt: ir.Statement, args: tuple[JointResult, ...]
    ) -> interp.Result[JointResult]:
        # Check if the statement has a custom implementation
        signature = self.build_signature(stmt, args)
        if signature in self.registry:
            return self.registry[signature](self, stmt, args)
        elif stmt.__class__ in self.registry:
            return self.registry[stmt.__class__](self, stmt, args)

        # Otherwise, use the default implementation
        const_results = self.constprop.eval_stmt(stmt, tuple(x.const for x in args))
        type_results = self.typeinfer.eval_stmt(stmt, tuple(x.typ for x in args))
        frame = self.state.current_frame()

        if frame.extra is not None and frame.extra.frame_not_pure:
            purity = NotPure()
        elif stmt.has_trait(ir.Pure):
            purity = Pure()
        else:
            if frame.extra is None:
                frame.extra = ExtraFrameInfo(frame_not_pure=True)
            purity = NotPure()
        return self.wrap_results(type_results, const_results, purity)

    def wrap_results(
        self,
        type_results: interp.Result[ir.types.TypeAttribute],
        const_results: interp.Result[const.Result],
        purity: Pure | NotPure,
    ) -> interp.Result[JointResult]:
        match const_results, type_results:
            case (interp.ResultValue(), interp.ResultValue()):
                return interp.ResultValue(
                    *tuple(
                        JointResult(typ, const, purity)
                        for typ, const in zip(type_results.values, const_results.values)
                    )
                )
            case (interp.ReturnValue(), interp.ReturnValue()):
                return interp.ReturnValue(
                    JointResult(type_results.result, const_results.result, purity)
                )
            case (interp.Successor(), interp.Successor()):
                return interp.Successor(
                    block=type_results.block,
                    *tuple(
                        JointResult(typ, const, purity)
                        for typ, const in zip(
                            type_results.block_args, const_results.block_args
                        )
                    ),
                )
            case _:
                raise exceptions.InterpreterError(
                    "const.Propagate and TypeInference should return the same result type"
                )

    def run_method_region(
        self, mt: ir.Method, body: ir.Region, args: tuple[JointResult, ...]
    ) -> JointResult:
        const_result = self.constprop.run_method_region(
            mt, body, tuple(x.const for x in args)
        )
        type_result = self.typeinfer.run_method_region(
            mt, body, tuple(x.typ for x in args)
        )
        frame = self.state.current_frame()
        if frame.extra is not None and frame.extra.frame_not_pure:
            purity = NotPure()
        else:
            purity = Pure()
        return JointResult(type_result, const_result, purity)
