from dataclasses import dataclass

from kirin.dialects.func import Signature
from kirin.dialects.py import types
from kirin.ir import (
    Block,
    CallableStmtInterface,
    HasSignature,
    SSAValue,
    Statement,
    TypeAttribute,
)
from kirin.rewrite import RewriteResult, RewriteRule


@dataclass
class ApplyType(RewriteRule):
    ret_type: TypeAttribute
    results: dict[SSAValue, TypeAttribute]

    def get_type(self, value: SSAValue) -> TypeAttribute:
        typ = self.results[value]
        if isinstance(typ, types.PyConst):
            return typ.typ
        return typ

    def rewrite_Block(self, node: Block) -> RewriteResult:
        has_done_something = False
        for arg in node.args:
            if arg in self.results:
                arg.type = self.get_type(arg)
                has_done_something = True

        return RewriteResult(has_done_something=has_done_something)

    def rewrite_Statement(self, node: Statement) -> RewriteResult:
        if (fn_trait := node.get_trait(HasSignature)) and (
            call_trait := node.get_trait(CallableStmtInterface)
        ):
            signature = fn_trait.get_signature(node)
            body = call_trait.get_callable_region(node)
            inputs = [
                self.get_type(arg) if arg in self.results else signature.inputs[idx]
                for idx, arg in enumerate(body.blocks[0].args[1:])
            ]
            fn_trait.set_signature(node, Signature(tuple(inputs), self.ret_type))

        has_done_something = False
        for result in node._results:
            if result in self.results:
                result.type = self.get_type(result)
                has_done_something = True

        return RewriteResult(has_done_something=has_done_something)
