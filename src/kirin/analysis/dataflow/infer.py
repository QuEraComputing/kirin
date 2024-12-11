from dataclasses import dataclass
from typing import Iterable

from kirin import exceptions, interp, ir

from .constprop import ConstProp
from .forward import ForwardExtra
from .lattice import InferenceLattice
from .lattice.const import ConstLattice
from .lattice.purity import NotPure, Pure
from .typeinfer import TypeInference


@dataclass
class ExtraFrameInfo:
    frame_not_pure: bool = False


@dataclass
class Inference(ForwardExtra[InferenceLattice, ExtraFrameInfo]):
    keys = ["inference", "empty"]
    lattice = InferenceLattice

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
        self.constprop = ConstProp(
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
        self, stmt: ir.Statement, args: tuple[InferenceLattice, ...]
    ) -> interp.Result[InferenceLattice]:
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
        const_results: interp.Result[ConstLattice],
        purity: Pure | NotPure,
    ) -> interp.Result[InferenceLattice]:
        match const_results, type_results:
            case (interp.ResultValue(), interp.ResultValue()):
                return interp.ResultValue(
                    *tuple(
                        InferenceLattice(typ, const, purity)
                        for typ, const in zip(type_results.values, const_results.values)
                    )
                )
            case (interp.ReturnValue(), interp.ReturnValue()):
                return interp.ReturnValue(
                    InferenceLattice(type_results.result, const_results.result, purity)
                )
            case (interp.Successor(), interp.Successor()):
                return interp.Successor(
                    block=type_results.block,
                    *tuple(
                        InferenceLattice(typ, const, purity)
                        for typ, const in zip(
                            type_results.block_args, const_results.block_args
                        )
                    ),
                )
            case _:
                raise exceptions.InterpreterError(
                    "ConstProp and TypeInference should return the same result type"
                )

    def run_method_region(
        self, mt: ir.Method, body: ir.Region, args: tuple[InferenceLattice, ...]
    ) -> InferenceLattice:
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
        return InferenceLattice(type_result, const_result, purity)
