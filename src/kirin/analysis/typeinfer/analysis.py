from __future__ import annotations

from typing import TYPE_CHECKING, final

from kirin import ir, types, interp
from kirin.decl import fields
from kirin.analysis.forward import Forward, ForwardFrame

from .solve import TypeResolution

if TYPE_CHECKING:
    from kirin.dialects.func.attrs import Signature


@final
class TypeInference(Forward[types.TypeAttribute]):
    """Type inference analysis for kirin.

    This analysis uses the forward dataflow analysis framework to infer the types of
    the IR. The analysis uses the type information within the IR to determine the
    method dispatch.

    The analysis will fallback to a type resolution algorithm if the type information
    is not available in the IR but the type information is available in the abstract
    values.
    """

    keys = ("typeinfer",)
    lattice = types.TypeAttribute

    def run(self, method: ir.Method, *args, **kwargs):
        if not args and not kwargs:  # no args or kwargs
            # use the method signature to get the args
            args = method.arg_types
        return super().run(method, *args, **kwargs)

    def method_self(self, method: ir.Method) -> types.TypeAttribute:
        return method.self_type

    def frame_call(
        self,
        frame: ForwardFrame[types.TypeAttribute],
        node: ir.Statement,
        *args: types.TypeAttribute,
        **kwargs: types.TypeAttribute,
    ) -> types.TypeAttribute:
        trait = node.get_present_trait(ir.CallableStmtInterface)
        args = trait.align_input_args(node, *args, **kwargs)
        region = trait.get_callable_region(node)
        if self.state.depth >= self.max_depth:
            return self.recursion_limit_reached()

        if trait := node.get_trait(ir.HasSignature):
            signature: Signature[types.TypeAttribute] | None = trait.get_signature(node)
            args = (args[0],) + tuple(
                input.meet(arg) for input, arg in zip(signature.inputs, args[1:])
            )
        else:
            signature = None
        ret = self.frame_call_region(frame, node, region, *args)

        if isinstance(ret, interp.ReturnValue):
            return ret.value if signature is None else ret.value.meet(signature.output)
        elif not ret:  # empty result or None
            return self.void if signature is None else self.void.meet(signature.output)
        raise interp.InterpreterError(
            f"callable region {node.name} does not return `ReturnValue`, got {ret}"
        )

    def eval_fallback(
        self, frame: ForwardFrame[types.TypeAttribute], node: ir.Statement
    ) -> interp.StatementResult[types.TypeAttribute]:
        resolve = TypeResolution()
        fs = fields(node)
        for f, value in zip(fs.args.values(), frame.get_values(node.args)):
            resolve.solve(f.type, value)
        for arg, f in zip(node.args, fs.args.values()):
            frame.set(arg, frame.get(arg).meet(resolve.substitute(f.type)))
        return tuple(resolve.substitute(result.type) for result in node.results)

    # NOTE: unlike concrete interpreter, instead of using type information
    # within the IR. Type inference will use the interpreted
    # value (which is a type) to determine the method dispatch.
    def build_signature(
        self, frame: ForwardFrame[types.TypeAttribute], node: ir.Statement
    ) -> interp.Signature:
        argtypes = ()
        for x in frame.get_values(node.args):
            if isinstance(x, types.Generic):
                argtypes += (x.body,)
            else:
                argtypes += (x,)
        return interp.Signature(type(node), argtypes)
