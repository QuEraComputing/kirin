from kirin.codegen import CodeGen, DialectEmit, impl

from .attrs import Signature
from .dialect import dialect


@dialect.register(key="dict")
class EmitDict(DialectEmit[CodeGen[dict], dict]):

    @impl(Signature)
    def emit_signature(self, emit: CodeGen[dict], stmt: Signature):
        return {
            "name": stmt.name,
            "inputs": [emit.emit_Attribute(arg) for arg in stmt.inputs],
            "output": emit.emit_Attribute(stmt.output),
        }
