from dataclasses import dataclass, field
from typing import Iterable

from kirin import ir
from kirin.exceptions import CodeGenError

from .base import CodeGen
from .ssa import IdTable


@dataclass(init=False)
class DictGen(CodeGen[dict]):
    keys = ["dict", "str"]
    root: dict = field(init=False)
    ssa_id: IdTable[ir.SSAValue] = field(default_factory=IdTable[ir.SSAValue])
    block_id: IdTable[ir.Block] = field(default_factory=IdTable[ir.Block])

    def __init__(self, dialects: ir.DialectGroup | Iterable[ir.Dialect]):
        super().__init__(dialects)
        self.ssa_id = IdTable()
        self.block_id = IdTable()

    def emit(self, mt: ir.Method):
        self.root = {
            "globals": {},
            "methods": {},
            "entry": mt.sym_name,
        }
        self.root[mt.sym_name] = self.emit_Method(mt)
        return self.root

    def emit_Method(self, mt: ir.Method):
        return {
            "name": mt.sym_name,
            "args": {
                "names": mt.arg_names,
                "types": [self.emit_Attribute(typ) for typ in mt.arg_types],
            },
            "body": self.emit_Statement(mt.code),
        }

    def emit_Region(self, region: ir.Region):
        return {
            "type": "ir.region",
            "blocks": [self.emit_Block(b) for b in region.blocks],
        }

    def emit_Block(self, block: ir.Block) -> dict:
        return {
            "type": "ir.block",
            "id": self.block_id[block],
            "stmts": [self.emit_Statement(stmt) for stmt in block.stmts],
        }

    def emit_Statement_fallback(self, stmt: ir.Statement) -> dict:
        if stmt.dialect is None:
            raise CodeGenError(
                f"Dialect is not set for statement {stmt.__class__.__name__}"
            )

        return {
            "dialect": stmt.dialect.name,
            "type": stmt.__class__.__name__,
            "args": [self.ssa_id[a] for a in stmt.args],
            "results": [self.ssa_id[r] for r in stmt.results],
            "successors": [self.block_id[b] for b in stmt.successors],
            "regions": [self.emit_Region(r) for r in stmt.regions],
            "properties": {
                name: self.emit_Attribute(attr)
                for name, attr in stmt.properties.items()
            },
            "attributes": {
                name: self.emit_Attribute(attr)
                for name, attr in stmt.attributes.items()
            },
        }
