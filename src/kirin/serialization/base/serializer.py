from typing import Any
from dataclasses import field

from kirin import ir, types
from kirin.dialects import func, ilist
from kirin.serialization.base.context import SerializationContext
from kirin.serialization.base.registry import (
    DIALECTS_LOOKUP,
    DialectSerializer,
    RuntimeSerializer,
    TypeAttributeSerializer,
    mangle,
    register_dialect,
)


class Serializer:
    _ctx: SerializationContext
    _runtime_serializer: RuntimeSerializer = field(default_factory=RuntimeSerializer)
    _typeattr_serializer: TypeAttributeSerializer = field(
        default_factory=TypeAttributeSerializer
    )
    _dialect_serializer: DialectSerializer = field(default_factory=DialectSerializer)
    _block_reference_store: dict[int, ir.Block]

    def __init__(self):
        self._ctx = SerializationContext()
        self._runtime_serializer = RuntimeSerializer()
        self._typeattr_serializer = TypeAttributeSerializer()
        self._dialect_serializer = DialectSerializer()
        self._block_reference_store = {}

    def encode(self, obj: object) -> dict[str, Any]:
        out = {"kind": "module", "symbol_table": None, "body": self.serialize(obj)}
        return out

    def decode(self, data: dict[str, Any]) -> Any:
        kind = data.get("kind")
        self._block_reference_store = {}
        for i in range(len(self._ctx.blk_idtable.lookup)):
            x = ir.Block.__new__(ir.Block)
            self._block_reference_store[i] = x
        match kind:
            case "module":
                return self.deserialize(data["body"])
            case _:
                raise ValueError(
                    f"Unsupported data kind {kind} for module-level deserialization."
                )

    def serialize(self, obj: object) -> dict[str, Any]:
        match obj:
            case ir.Method():
                return self.serialize_method(obj)
            case ir.BlockArgument():
                return self.serialize_block_argument(obj)
            case ir.Statement():
                return self.serialize_statement(obj)
            case ir.Region():
                return self.serialize_region(obj)
            case ir.Attribute():
                return self.serialize_attribute(obj)
            case ir.Block():
                return self.serialize_block(obj)
            case ir.ResultValue():
                return self.serialize_result(obj)
            case _:
                raise ValueError(
                    f"Unsupported object type {type(obj)} for serialization."
                )

    def deserialize(
        self, data: dict[str, Any], owner: ir.Statement | None = None
    ) -> Any:
        kind = data.get("kind")
        match kind:
            case "method":
                return self.deserialize_method(data)
            case "block-arg":
                return self.deserialize_block_argument(data)
            case "statement":
                return self.deserialize_statement(data)
            case "region":
                return self.deserialize_region(data)
            case "region_ref":
                return self.deserialize_region(data)
            case "attribute":
                return self.deserialize_attribute(data)
            case "block" | "block_ref":
                return self.deserialize_block(data)
            case "result-value":
                return self.deserialize_result(data, owner=owner)
            case _:
                raise ValueError(f"Unsupported data kind {kind} for deserialization.")

    def serialize_method(self, mthd: ir.Method) -> dict[str, Any]:
        method_dialects = mthd.dialects
        if isinstance(method_dialects, ir.Dialect):
            register_dialect(method_dialects)
        elif isinstance(method_dialects, ir.DialectGroup):
            for d in method_dialects.data:
                register_dialect(d)

        mangled = mangle(mthd.sym_name, mthd.arg_types)
        return {
            "kind": "method",
            "sym_name": mthd.sym_name,
            "arg_names": mthd.arg_names,
            "dialects": self._dialect_serializer.encode(mthd.dialects),
            "code": self.serialize(mthd.code),
            "mangled": mangled,
        }

    def deserialize_method(self, data: dict[str, Any]) -> ir.Method:

        if data.get("kind") != "method":
            raise ValueError("Invalid method data for deserialization.")
        out = ir.Method(
            mod=None,
            py_func=None,
            sym_name=data["sym_name"],
            arg_names=data["arg_names"],
            dialects=self._dialect_serializer.decode(data["dialects"]),
            code=self.deserialize(data["code"]),
        )
        mangled = mangle(out.sym_name, out.arg_types)
        if mangled != data["mangled"]:
            raise ValueError(
                f"Mangled name mismatch: expected {data['mangled']}, got {mangled}"
            )
        self._ctx.Method_SymbolTable[mangled] = out
        return out

    def serialize_statement(self, stmt: ir.Statement) -> dict[str, Any]:
        dialects = stmt.dialect
        out = {
            "kind": "statement",
            "dialect": self._dialect_serializer.encode(dialects),
            "name": stmt.name,
            "_args": list(self.serialize(arg) for arg in stmt._args),
            "_results": list(self.serialize(res) for res in stmt._results),
            "_name_args_slice": stmt._name_args_slice,
            "attributes": {k: self.serialize(v) for k, v in stmt.attributes.items()},
            "successors": [self.serialize(succ) for succ in stmt.successors],
            "_regions": [self.serialize(region) for region in stmt._regions],
        }
        if isinstance(stmt, func.Invoke):
            out["call_method"] = mangle(stmt.callee.sym_name, stmt.callee.arg_types)
        else:
            out["call_method"] = None
        return out

    def deserialize_statement(self, data: dict[str, Any]) -> ir.Statement:
        if data.get("kind") != "statement":
            raise ValueError("Invalid statement data for decoding.")

        dialect_name = data["dialect"]["name"]
        tmp = DIALECTS_LOOKUP.get(dialect_name)

        if tmp is None:
            raise ValueError(f"Dialect {dialect_name} not found in lookup.")

        dialect, stmt_map = tmp
        stmt_name = data["name"]
        stmt_cls = stmt_map.get(stmt_name)

        if stmt_cls is None:
            raise ValueError(
                f"Statement class {stmt_name} not found in dialect {dialect_name}."
            )

        out = stmt_cls.__new__(stmt_cls)
        _args = tuple(self.deserialize(x) for x in data["_args"])
        _results = list(self.deserialize(owner=out, data=x) for x in data["_results"])
        _name_args_slice = data["_name_args_slice"]
        _attributes = {k: self.deserialize(v) for k, v in data["attributes"].items()}

        out._args = _args
        out._results = _results
        out._name_args_slice = _name_args_slice
        out.attributes = _attributes

        successors_data = data.get("successors", [])
        out.successors = [self.deserialize(succ_data) for succ_data in successors_data]

        regions_data = data.get("_regions", [])
        _regions = [self.deserialize(region_data) for region_data in regions_data]

        if isinstance(out, func.Invoke) and data.get("call_method"):
            mangled_name = data["call_method"]
            if mangled_name not in self._ctx.Method_SymbolTable:
                raise ValueError(f"Method with mangled name {mangled_name} not found.")
            else:
                callee = self._ctx.Method_SymbolTable[mangled_name]
                out.callee = callee

        for region in _regions:
            if region.parent_node is None:
                region.parent_node = out
        out._regions = _regions

        return out

    def serialize_block_argument(self, arg: ir.BlockArgument) -> dict[str, Any]:

        out = {
            "kind": "block-arg",
            "id": self._ctx.ssa_idtable[arg],
            "blk_id": self._ctx.blk_idtable[arg.owner],
            "index": arg.index,
            "type": self._typeattr_serializer.encode(arg.type),
            "name": arg.name,
        }
        return out

    def deserialize_block_argument(self, data: dict[str, Any]) -> ir.BlockArgument:
        if data.get("kind") != "block-arg":
            raise ValueError("Invalid SSA block argument data for decoding.")

        ssa_id = int(data["id"])
        if ssa_id in self._ctx.SSA_Lookup:
            existing = self._ctx.SSA_Lookup[ssa_id]
            if isinstance(existing, ir.BlockArgument):
                return existing
            raise ValueError(
                f"Block argument id {ssa_id} already present but maps to {type(existing).__name__}"
            )

        block = self._ctx.Block_Lookup.get(int(data["blk_id"]))
        if block is None:
            block_id = int(data["blk_id"])
            if block_id in self._block_reference_store:
                block = self._block_reference_store.pop(block_id)
                self._ctx.Block_Lookup[block_id] = block
            else:
                raise ValueError(f"Block with id {block_id} not found in lookup.")

        index = data["index"]

        typ = self._typeattr_serializer.decode(data["type"])
        out = ir.BlockArgument(block=block, index=index, type=typ)
        out._name = data.get("name", None)
        self._ctx.SSA_Lookup[ssa_id] = out  # reg to ssa lookup

        return out

    def serialize_region(self, region: ir.Region) -> dict[str, Any]:
        region_id = self._ctx.region_idtable[region]
        if region_id in self._ctx.Region_Lookup:
            out = {
                "kind": "region_ref",
                "id": region_id,
            }
        else:
            self._ctx.Region_Lookup[region_id] = region
            out = {
                "kind": "region",
                "id": region_id,
                "blocks": [self.serialize(block) for block in region.blocks],
            }
        return out

    def deserialize_region(self, data: dict[str, Any]) -> ir.Region:
        if data.get("kind") == "region":
            out = ir.Region.__new__(ir.Region)
            self._ctx.Region_Lookup[self._ctx.region_idtable[out]] = out

            blocks = [self.deserialize(blk) for blk in data.get("blocks", [])]

            out._blocks = []
            out._block_idx = {}

            for block in blocks:
                existing_parent = block.parent
                if existing_parent is not None and existing_parent is not out:
                    block.parent = None
                out.blocks.append(block)

            return out
        elif data.get("kind") == "region_ref":
            region_id = int(data["id"])
            if region_id not in self._ctx.Region_Lookup:
                raise ValueError(f"Region with id {region_id} not found in lookup.")

            return self._ctx.Region_Lookup[region_id]
        else:
            raise ValueError("Invalid region data for decoding.")

    def serialize_block(self, block: ir.Block) -> dict[str, Any]:
        if self._ctx.blk_idtable[block] in self._ctx.Block_Lookup:
            out = {
                "kind": "block_ref",
                "id": self._ctx.blk_idtable[block],
            }
        else:
            self._ctx.Block_Lookup[self._ctx.blk_idtable[block]] = block
            out = {
                "kind": "block",
                "id": self._ctx.blk_idtable[block],
                "stmts": [self.serialize_statement(stmt) for stmt in block.stmts],
                "_args": [self.serialize_block_argument(arg) for arg in block.args],
            }
        return out

    def deserialize_block(self, block_data: dict) -> ir.Block:
        if block_data.get("kind") == "block_ref":
            return self.deserialize_block_ref(block_data)
        elif block_data.get("kind") == "block":
            return self.deserialize_concrete_block(block_data)
        else:
            raise ValueError("Invalid block data for decoding.")

    def deserialize_block_ref(self, block_data: dict) -> ir.Block:
        if block_data.get("kind") != "block_ref":
            raise ValueError("Invalid block reference data for decoding.")

        block_id = int(block_data["id"])
        if block_id not in self._ctx.Block_Lookup:
            raise ValueError(f"Block with id {block_id} not found in lookup.")

        return self._ctx.Block_Lookup[block_id]

    def deserialize_concrete_block(self, block_data: dict) -> ir.Block:
        if block_data.get("kind") != "block":
            raise ValueError("Invalid block data for decoding.")

        block_id = int(block_data["id"])

        if block_id not in self._ctx.Block_Lookup:
            if block_id in self._block_reference_store:
                out = self._block_reference_store.pop(block_id)
                self._ctx.Block_Lookup[block_id] = out
            raise ValueError(f"Block with id {block_id} not found in lookup.")
        else:
            out = self._ctx.Block_Lookup[block_id]

        out._args = tuple(
            self.deserialize_block_argument(arg_data)
            for arg_data in block_data.get("_args", [])
        )

        stmts_data = block_data.get("stmts")
        if stmts_data is None:
            raise ValueError("Block data must contain 'stmts' field.")

        out._first_stmt = None
        out._last_stmt = None
        out._first_branch = None
        out._last_branch = None
        out._stmt_len = 0
        stmts = tuple(self.deserialize_statement(stmt_data) for stmt_data in stmts_data)
        out.stmts.extend(stmts)

        return out

    def serialize_attribute(self, attr: ir.Attribute) -> dict[str, Any]:
        out: dict[str, Any] = {
            "kind": "attribute",
        }
        if isinstance(attr, ir.PyAttr):
            if isinstance(attr.data, ir.Method):
                out["style"] = "pyattr-method"
                out["data"] = self.serialize_method(attr.data)
            else:
                out["style"] = "pyattr"
                out["data"] = self._runtime_serializer.encode(attr.data)
            out["type"] = self._typeattr_serializer.encode(attr.type)
        elif isinstance(attr, ilist.IList):
            out["style"] = "ilist"
            out["data"] = self._runtime_serializer.encode(attr.data)
        elif isinstance(attr, func.Signature):
            out["style"] = "signature"
            out["inputs"] = [
                self._typeattr_serializer.encode(arg) for arg in attr.inputs
            ]
            out["output"] = self._typeattr_serializer.encode(attr.output)
        elif isinstance(attr, types.TypeAttribute):
            out["style"] = "typeattr"
            out["data"] = self._typeattr_serializer.encode(attr)
        else:
            raise ValueError(f"Unsupported attribute type {type(attr)} for encoding.")

        return out

    def deserialize_attribute(self, data: dict[str, Any]) -> ir.Attribute:
        if data.get("kind") != "attribute":
            raise ValueError("Invalid attribute data for decoding.")

        style = data.get("style")
        if style == "pyattr":
            return ir.PyAttr(
                data=self._runtime_serializer.decode(data["data"]),
                pytype=self._typeattr_serializer.decode(data["type"]),
            )
        elif style == "pyattr-method":
            return ir.PyAttr(
                data=self.deserialize_method(data["data"]),
                pytype=self._typeattr_serializer.decode(data["type"]),
            )
        elif style == "ilist":
            return ilist.IList(data=self._runtime_serializer.decode(data["data"]))
        elif style == "signature":
            inputs = [self._typeattr_serializer.decode(arg) for arg in data["inputs"]]
            output = self._typeattr_serializer.decode(data["output"])
            return func.Signature(inputs=tuple(inputs), output=output)
        elif style == "typeattr":
            return self._typeattr_serializer.decode(data["data"])
        else:
            raise ValueError(f"Unsupported attribute <{style}> for decoding.")

    def serialize_result(self, result: ir.ResultValue) -> dict[str, Any]:
        return {
            "kind": "result-value",
            "id": self._ctx.ssa_idtable[result],
            "index": result.index,
            "type": self._typeattr_serializer.encode(result.type),
            "name": result.name,
        }

    def deserialize_result(
        self, data: dict[str, Any], owner: ir.Statement
    ) -> ir.ResultValue:
        if data.get("kind") != "result-value":
            raise ValueError("Invalid result SSA data for decoding.")
        ssa_id = int(data["id"])

        if ssa_id in self._ctx.SSA_Lookup:
            existing = self._ctx.SSA_Lookup[ssa_id]
            if isinstance(existing, ir.ResultValue):
                return existing
            raise ValueError(
                f"SSA id {ssa_id} already exists and is {type(existing).__name__}"
            )

        index = int(data["index"])

        typ = self._typeattr_serializer.decode(data["type"])

        out = ir.ResultValue(stmt=owner, index=index, type=typ)
        out.name = data.get("name", None)

        self._ctx.SSA_Lookup[ssa_id] = out

        return out
