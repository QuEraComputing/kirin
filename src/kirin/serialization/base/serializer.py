from typing import Any

from kirin import ir
from kirin.dialects import func, ilist
from kirin.serialization.base.context import SerializationContext
from kirin.serialization.base.registry import DIALECTS_LOOKUP, register_dialect


class Serializer:
    def __init__(
        self,
        ctx: SerializationContext | None = None,
        dialects: ir.Dialect | ir.DialectGroup | None = None,
    ):
        self._ctx = ctx or SerializationContext()
        if dialects:
            if isinstance(dialects, ir.Dialect):
                items = [dialects]
            elif isinstance(dialects, ir.DialectGroup):
                items = [d for d in dialects.data]

            for d in items:
                register_dialect(d)

    def serialize_method(self, mthd: ir.Method) -> dict[str, Any]:
        self._ctx.clear()
        # Ensure all dialects referenced by the method are registered so
        # DialectSerializer.encode can look them up. Methods may carry a
        # Dialect or a DialectGroup containing multiple dialects.
        try:
            method_dialects = mthd.dialects
        except AttributeError:
            method_dialects = None

        if isinstance(method_dialects, ir.Dialect):
            register_dialect(method_dialects)
        elif isinstance(method_dialects, ir.DialectGroup):
            for d in method_dialects.data:
                register_dialect(d)
        return {
            "kind": "method",
            "sym_name": mthd.sym_name,
            "arg_names": mthd.arg_names,
            "dialects": self._ctx.dialect_serializer.encode(mthd.dialects),
            "code": self.serialize(mthd.code),
        }

    def deserialize_method(self, data: dict[str, Any]) -> ir.Method:
        return ir.Method(
            mod=None,
            py_func=None,
            sym_name=data["sym_name"],
            arg_names=data["arg_names"],
            dialects=self._ctx.dialect_serializer.decode(data["dialects"]),
            code=self.deserialize(data["code"]),
        )

    def serialize(self, obj: object) -> dict[str, Any]:
        match obj:
            case ir.BlockArgument():
                return self.serialize_block_argument(obj)
            # case ir.SSAValue():
            #     return self.serialize_ssa_value(obj)
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

    def deserialize(self, data: dict[str, Any], owner: ir.Statement = None) -> Any:
        kind = data.get("kind")
        match kind:
            case "block-arg":
                return self.deserialize_block_argument(data)
            # case "ssa-value":
            #     return self.deserialize_ssa_value(data)
            case "statement":
                return self.deserialize_statement(data)
            case "region":
                return self.deserialize_region(data)
            case "attribute":
                return self.deserialize_attribute(data)
            case "block":
                return self.deserialize_block(data)
            case "result-value":
                return self.deserialize_result(data, owner=owner)
            case _:
                raise ValueError(f"Unsupported data kind {kind} for deserialization.")

    def serialize_statement(self, stmt: ir.Statement) -> dict[str, Any]:
        out = {
            "kind": "statement",
            "dialect": self._ctx.dialect_serializer.encode(
                getattr(stmt, "dialect", None)
            ),
            "name": stmt.name,
            "_args": list(self.serialize(arg) for arg in stmt._args),
            "_results": list(self.serialize(res) for res in stmt._results),
            "_name_args_slice": stmt._name_args_slice,
            "attributes": {k: self.serialize(v) for k, v in stmt.attributes.items()},
            "successors": [self.serialize(succ) for succ in stmt.successors],
            "_regions": [self.serialize(region) for region in stmt._regions],
        }
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

        # initialize the instance:
        out = stmt_cls.__new__(stmt_cls)

        # decode fields:
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

        # deal with :
        regions_data = data.get("_regions", [])
        _regions = [self.deserialize(region_data) for region_data in regions_data]
        # link parents:
        for region in _regions:
            if region.parent_node is None:
                region.parent_node = out
        out._regions = _regions

        return out

    def serialize_block_argument(self, arg: ir.BlockArgument) -> dict[str, Any]:
        return {
            "kind": "block-arg",
            "id": self._ctx.ssa_idtable[arg],
            "blk_id": self._ctx.blk_idtable[arg.owner],
            "index": arg.index,
            "type": self._ctx.typeattr_serializer.encode(arg.type),
            "name": arg.name,
        }

    def deserialize_block_argument(self, data: dict[str, Any]) -> ir.BlockArgument:
        if data.get("kind") != "block-arg":
            raise ValueError("Invalid SSA block argument data for decoding.")

        ssa_id = int(data["id"])

        # If this SSA id was already created earlier in the decode (e.g. the
        # same block-arg was referenced multiple times in the serialized
        # payload), return the existing object.
        if ssa_id in self._ctx.SSA_Lookup:
            existing = self._ctx.SSA_Lookup[ssa_id]
            if isinstance(existing, ir.BlockArgument):
                return existing
            raise ValueError(
                f"Block argument id {ssa_id} already present but maps to {type(existing).__name__}"
            )

        block = self._ctx.Block_Lookup.get(int(data["blk_id"]))
        if block is None:
            raise ValueError(f"Block with id {data['blk_id']} not found in lookup.")

        index = data["index"]

        typ = self._ctx.typeattr_serializer.decode(data["type"])

        # construct BlockArgument:
        out = ir.BlockArgument(block=block, index=index, type=typ)
        out._name = data.get("name", None)
        self._ctx.SSA_Lookup[ssa_id] = out  # reg to ssa lookup

        return out

    # def serialize_ssa_value(self, value: ir.SSAValue) -> dict[str, Any]:
    #     return {"kind": "ssa-value", "id": self._ctx.ssa_idtable[value]}

    # def deserialize_ssa_value(self, data: dict[str, Any]) -> ir.SSAValue:
    #     if data.get("kind") != "ssa-value":
    #         raise ValueError("Invalid SSA data for decoding.")

    #     ssa_id = int(data["id"])
    #     out = self._ctx.SSA_Lookup.get(ssa_id)
    #     if out is None:
    #         raise ValueError(f"SSA value with id {ssa_id} not found in lookup.")

    #     return out

    def serialize_region(self, region: ir.Region) -> dict[str, Any]:
        region_id = self._ctx.region_idtable[region]
        if region_id in self._ctx.Region_Lookup:
            out = {
                "kind": "region_ref",
                "id": region_id,
            }
        else:
            # register region and include explicit id so decoder can reuse it
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
            self._ctx.Block_Lookup[self._ctx.blk_idtable[block]] = (
                block  # register to Block lookup
            )
            out = {
                "kind": "block",
                "id": self._ctx.blk_idtable[block],
                "stmts": [self.serialize(stmt) for stmt in block.stmts],
                "_args": [self.serialize(arg) for arg in block.args],
            }
        return out

    def deserialize_block(self, data: dict[str, Any]) -> ir.Block:
        if data.get("kind") == "block_ref":
            block_id = int(data["id"])
            if block_id not in self._ctx.Block_Lookup:
                raise ValueError(f"Block with id {block_id} not found in lookup.")

            return self._ctx.Block_Lookup.get(block_id)
        elif data.get("kind") == "block":
            block_id = int(data["id"])

            out = ir.Block.__new__(ir.Block)
            self._ctx.Block_Lookup[block_id] = (
                out  # register to block_id first, so the consecutive ref can follow
            )

            # construct the block:
            out._args = tuple(
                self.deserialize(arg_data) for arg_data in data.get("_args", [])
            )

            stmts_data = data.get("stmts")
            if stmts_data is None:
                raise ValueError("Block data must contain 'stmts' field.")

            out._first_stmt = None
            out._last_stmt = None
            out._first_branch = None
            out._last_branch = None
            out._stmt_len = 0
            stmts = tuple(self.deserialize(stmt_data) for stmt_data in stmts_data)
            out.stmts.extend(stmts)

            return out
        else:
            raise ValueError("Invalid block data for decoding.")

    def serialize_attribute(self, attr: ir.Attribute) -> dict[str, Any]:
        out: dict[str, Any] = {
            "kind": "attribute",
        }
        if isinstance(attr, ir.PyAttr):
            out["style"] = "pyattr"
            out["data"] = self._ctx.runtime_serializer.encode(attr.data)
        elif isinstance(attr, ilist.IList):
            out["style"] = "ilist"
            out["data"] = self._ctx.runtime_serializer.encode(attr.data)
        elif isinstance(attr, func.Signature):
            out["style"] = "signature"
            out["inputs"] = [
                self._ctx.typeattr_serializer.encode(arg) for arg in attr.inputs
            ]
            out["output"] = self._ctx.typeattr_serializer.encode(attr.output)

        else:
            raise ValueError(f"Unsupported attribute type {type(attr)} for encoding.")

        return out

    def deserialize_attribute(self, data: dict[str, Any]) -> ir.Attribute:
        if data.get("kind") != "attribute":
            raise ValueError("Invalid attribute data for decoding.")

        style = data.get("style")
        if style == "pyattr":
            return ir.PyAttr(data=self._ctx.runtime_serializer.decode(data["data"]))
        elif style == "ilist":
            return ilist.IList(data=self._ctx.runtime_serializer.decode(data["data"]))
        elif style == "signature":
            inputs = [
                self._ctx.typeattr_serializer.decode(arg) for arg in data["inputs"]
            ]
            output = self._ctx.typeattr_serializer.decode(data["output"])
            return func.Signature(inputs=tuple(inputs), output=output)
        else:
            raise ValueError(f"Unsupported attribute <{style}> for decoding.")

    def serialize_result(self, result: ir.ResultValue) -> dict[str, Any]:
        return {
            "kind": "result-value",
            "id": self._ctx.ssa_idtable[result],
            "index": result.index,
            "type": self._ctx.typeattr_serializer.encode(result.type),
            "name": result.name,
        }

    def deserialize_result(
        self, data: dict[str, Any], owner: ir.Statement
    ) -> ir.ResultValue:
        if data.get("kind") != "result-value":
            raise ValueError("Invalid result SSA data for decoding.")
        ssa_id = int(data["id"])
        # If the id was already created earlier (multiple references to the
        # same SSA value in the serialized payload), reuse the existing object
        # if it's the right kind.
        if ssa_id in self._ctx.SSA_Lookup:
            existing = self._ctx.SSA_Lookup[ssa_id]
            if isinstance(existing, ir.ResultValue):
                return existing
            raise ValueError(
                f"SSA id {ssa_id} already exists and is {type(existing).__name__}"
            )

        index = int(data["index"])

        typ = self._ctx.typeattr_serializer.decode(data["type"])

        out = ir.ResultValue(stmt=owner, index=index, type=typ)
        out.name = data.get("name", None)

        self._ctx.SSA_Lookup[ssa_id] = out

        return out
