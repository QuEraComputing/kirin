from typing import Any, cast
from dataclasses import field, dataclass

from kirin import ir
from kirin.serialization.base.context import (
    MethodSymbolMeta,
    SerializationContext,
    mangle,
    get_cls_from_name,
)
from kirin.serialization.core.deserializable import Deserializable
from kirin.serialization.core.serializationunit import SerializationUnit
from kirin.serialization.core.serializationmodule import SerializationModule


@dataclass
class Deserializer:
    dialect_group: ir.DialectGroup
    _ctx: SerializationContext = field(default_factory=SerializationContext, init=False)

    def decode(self, data: SerializationModule) -> ir.Method:
        self._ctx.clear()
        for mangled, meta in data.symbol_table.items():
            sym_name = meta.get("sym_name", None)
            if sym_name is None:
                raise ValueError(f"symbol_table[{mangled}] missing 'sym_name'")
            arg_types = meta.get("arg_types", []) or []
            self._ctx.Method_Symbol[mangled] = MethodSymbolMeta(
                sym_name=sym_name,
                arg_types=list(arg_types),
            )

        body = data.body
        if body is None:
            raise ValueError("Module envelope missing body for decoding.")
        return self.deserialize_method(body)

    def deserialize(self, serUnit: SerializationUnit) -> Any:
        if serUnit.kind == "attribute":
            return self.deserialize_attribute(serUnit)
        elif serUnit.kind == "type":
            return self.deserialize_type(serUnit)
        ser_method = getattr(
            self, "deserialize_" + serUnit.class_name.lower(), self.generic_deserialize
        )
        return ser_method(serUnit)

    def generic_deserialize(self, data: SerializationUnit) -> Any:
        if not hasattr(data, "kind"):
            raise ValueError(
                f"Invalid SerializationUnit: {data} missing 'kind' attribute."
            )
        match data.kind:
            case "bool":
                return self.deserialize_boolean(data)
            case "bytes":
                return self.deserialize_bytes(data)
            case "bytearray":
                return self.deserialize_bytearray(data)
            case "dict":
                return self.deserialize_dict(data)
            case "float":
                return self.deserialize_float(data)
            case "frozenset":
                return self.deserialize_frozenset(data)
            case "int":
                return self.deserialize_int(data)
            case "list":
                return self.deserialize_list(data)
            case "range":
                return self.deserialize_range(data)
            case "set":
                return self.deserialize_set(data)
            case "slice":
                return self.deserialize_slice(data)
            case "str":
                return self.deserialize_str(data)
            case "none":
                return self.deserialize_none(data)
            case "tuple":
                return self.deserialize_tuple(data)
            case "type":
                return self.deserialize_type(data)
            case _:
                obj = get_cls_from_name(serUnit=data)
                if isinstance(obj, Deserializable):
                    return obj.deserialize(data, self)
                else:
                    raise ValueError(
                        f"Unsupported kind {data.kind} for deserialization."
                    )

    def deserialize_method(self, serUnit: SerializationUnit) -> ir.Method:
        mangled = serUnit.data.get("mangled")
        if mangled is None:
            raise ValueError("Missing 'mangled' key for method deserialization.")

        out = self._ctx.Method_Runtime.get(mangled)
        if out is None:
            out = ir.Method.__new__(ir.Method)
            out.mod = None
            out.py_func = None
            out.code = ir.Statement.__new__(ir.Statement)
            self._ctx.Method_Runtime[mangled] = out

        out.sym_name = serUnit.data["sym_name"]
        out.arg_names = serUnit.data.get("arg_names", [])
        out.nargs = self.deserialize_int(serUnit.data["nargs"])
        out.dialects = self.dialect_group
        out.code = self.deserialize_statement(serUnit.data["code"])
        out.fields = self.deserialize_tuple(serUnit.data.get("fields", ()))
        computed = mangle(
            out.sym_name,
            getattr(out, "arg_types", ()),
            getattr(out, "ret_type", None),
        )
        if computed != mangled:
            raise ValueError(
                f"Mangled name mismatch: expected {mangled}, got {computed}"
            )
        return out

    def deserialize_statement(self, serUnit: SerializationUnit) -> ir.Statement:
        cls = get_cls_from_name(serUnit=serUnit)
        data = serUnit.data
        out = ir.Statement.__new__(cls)
        self._ctx.Statement_Lookup[data["id"]] = out
        out.dialect = self.deserialize(data["dialect"])
        out.name = self.deserialize_str(data["name"])
        out._args = self.deserialize_tuple(data["_args"])
        out._results = self.deserialize_list(data["_results"])
        out._name_args_slice = self.deserialize_dict(data["_name_args_slice"])
        out.attributes = self.deserialize_dict(data["attributes"])
        out.successors = self.deserialize_list(data["successors"])
        _regions = self.deserialize_list(data["_regions"])
        for region in _regions:
            if region.parent_node is None:
                region.parent_node = out
        out._regions = _regions
        return out

    def deserialize_blockargument(self, serUnit: SerializationUnit) -> ir.BlockArgument:
        cls = get_cls_from_name(serUnit=serUnit)
        ssa_name = serUnit.data["id"]
        if ssa_name in self._ctx.SSA_Lookup:
            existing = self._ctx.SSA_Lookup[ssa_name]
            if isinstance(existing, ir.BlockArgument):
                return existing
            raise ValueError(
                f"Block argument id {ssa_name} already present but maps to {type(existing).__name__}"
            )

        blk_name = serUnit.data["blk_id"]
        if blk_name not in self._ctx.Block_Lookup:
            block = ir.Block.__new__(ir.Block)
            self._ctx.Block_Lookup[blk_name] = block
        else:
            block = self._ctx.Block_Lookup[blk_name]
        index = serUnit.data["index"]
        typ = self.deserialize_attribute(serUnit.data["type"])
        out = cls(block=block, index=index, type=typ)
        out._name = serUnit.data.get("name", None)
        self._ctx.SSA_Lookup[ssa_name] = out

        return out

    def deserialize_region(self, serUnit: SerializationUnit) -> ir.Region:
        if serUnit.kind == "region":
            out = ir.Region.__new__(ir.Region)
            region_name = serUnit.data.get("id")
            if region_name is not None:
                self._ctx.Region_Lookup[region_name] = out

            blocks = [self.deserialize(blk) for blk in serUnit.data.get("blocks", [])]

            out._blocks = []
            out._block_idx = {}

            for block in blocks:
                existing_parent = block.parent
                if existing_parent is not None and existing_parent is not out:
                    block.parent = None
                out.blocks.append(block)

            return out
        elif serUnit.data.get("kind") == "region_ref":
            region_name = serUnit.data["id"]
            if region_name not in self._ctx.Region_Lookup:
                raise ValueError(f"Region with id {region_name} not found in lookup.")
            return self._ctx.Region_Lookup[region_name]
        else:
            raise ValueError("Invalid region data for decoding.")

    def deserialize_block(self, serUnit: SerializationUnit) -> ir.Block:
        if serUnit.kind == "block_ref":
            return self.deserialize_block_ref(serUnit)
        elif serUnit.kind == "block":
            return self.deserialize_concrete_block(serUnit)
        else:
            raise ValueError("Invalid block data for decoding.")

    def deserialize_block_ref(self, serUnit: SerializationUnit) -> ir.Block:
        if serUnit.kind != "block_ref":
            raise ValueError("Invalid block reference data for decoding.")

        block_name = serUnit.data["id"]
        if block_name not in self._ctx.Block_Lookup:
            raise ValueError(f"Block with id {block_name} not found in lookup.")
        return self._ctx.Block_Lookup[block_name]

    def deserialize_concrete_block(self, serUnit: SerializationUnit) -> ir.Block:
        if serUnit.kind != "block":
            raise ValueError("Invalid block data for decoding.")

        block_name = serUnit.data["id"]

        if block_name not in self._ctx.Block_Lookup:
            if block_name in self._ctx._block_reference_store:
                out = self._ctx._block_reference_store.pop(block_name)
                self._ctx.Block_Lookup[block_name] = out
            else:
                out = ir.Block.__new__(ir.Block)
                self._ctx.Block_Lookup[block_name] = out
        else:
            out = self._ctx.Block_Lookup[block_name]

        out._args = tuple(
            self.deserialize_blockargument(arg_data)
            for arg_data in serUnit.data.get("_args", [])
        )

        stmts_data = serUnit.data.get("stmts")
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

    def deserialize_boolean(self, serUnit: SerializationUnit) -> bool:
        return bool(serUnit.data["value"])

    def deserialize_bytes(self, serUnit: SerializationUnit) -> bytes:
        return bytes.fromhex(serUnit.data["value"])

    def deserialize_bytearray(self, serUnit: SerializationUnit) -> bytearray:
        return bytearray.fromhex(serUnit.data["value"])

    def deserialize_dict(self, serUnit: SerializationUnit) -> dict:
        keys = [self.deserialize(k) for k in serUnit.data.get("keys", [])]
        values = [self.deserialize(v) for v in serUnit.data.get("values", [])]
        return dict(zip(keys, values))

    def deserialize_float(self, serUnit: SerializationUnit) -> float:
        return float(serUnit.data["value"])

    def deserialize_frozenset(self, serUnit: SerializationUnit) -> frozenset:
        return frozenset(self.deserialize(x) for x in serUnit.data.get("value", []))

    def deserialize_int(self, serUnit: SerializationUnit) -> int:
        return int(serUnit.data["value"])

    def deserialize_list(self, serUnit: SerializationUnit) -> list:
        return [self.deserialize(x) for x in serUnit.data.get("value", [])]

    def deserialize_range(self, serUnit: SerializationUnit) -> range:
        start = self.deserialize(serUnit.data.get("start", 0))
        stop = self.deserialize(serUnit.data.get("stop", 0))
        step = self.deserialize(serUnit.data.get("step", 1))
        return range(start, stop, step)

    def deserialize_set(self, serUnit: SerializationUnit) -> set:
        return set(self.deserialize(x) for x in serUnit.data.get("value", []))

    def deserialize_slice(self, serUnit: SerializationUnit) -> slice:
        start = self.deserialize(serUnit.data["start"])
        stop = self.deserialize(serUnit.data["stop"])
        step = self.deserialize(serUnit.data["step"])
        return slice(start, stop, step)

    def deserialize_str(self, serUnit: SerializationUnit) -> str:
        return serUnit.data["value"]

    def deserialize_none(self, serUnit: SerializationUnit) -> None:
        return None

    def deserialize_tuple(self, serUnit: SerializationUnit) -> tuple:
        return tuple(self.deserialize(x) for x in serUnit.data.get("value", []))

    def deserialize_attribute(self, serUnit: SerializationUnit) -> ir.Attribute:
        if serUnit.kind != "attribute":
            raise ValueError(f"Expected kind='attribute', got {serUnit.kind}")

        inner = serUnit.data.get("data")
        if not isinstance(inner, SerializationUnit):
            raise ValueError("Attribute data must contain a SerializationUnit")
        if inner.kind not in ("type-attribute", "pyattr"):
            belong_to_dialect = None
            for dialect in self.dialect_group.data:
                if inner.module_name == dialect.name:
                    belong_to_dialect = dialect
                    break
            if belong_to_dialect is not None:
                for cls in belong_to_dialect.attrs:
                    if cls.__name__ == inner.class_name and isinstance(
                        cls, Deserializable
                    ):
                        return cast(ir.Attribute, cls.deserialize(inner, self))
                raise ValueError(
                    f"Attribute class {inner.class_name} not found in dialect {belong_to_dialect.name}"
                )
        cls = get_cls_from_name(inner)
        return cls.deserialize(inner, self)

    def deserialize_resultvalue(self, serUnit: SerializationUnit) -> ir.ResultValue:
        ssa_name = serUnit.data["id"]
        if ssa_name in self._ctx.SSA_Lookup:
            existing = self._ctx.SSA_Lookup[ssa_name]
            if isinstance(existing, ir.ResultValue):
                return existing
            raise ValueError(
                f"SSA id {ssa_name} already exists and is {type(existing).__name__}"
            )
        index = int(serUnit.data["index"])

        typ = self.deserialize(serUnit.data["type"])
        owner: ir.Statement = self._ctx.Statement_Lookup[
            self.deserialize_str(serUnit.data["owner"])
        ]
        out = ir.ResultValue(stmt=owner, index=index, type=typ)
        out.name = serUnit.data.get("name", None)

        self._ctx.SSA_Lookup[ssa_name] = out

        return out

    def deserialize_type(self, serUnit: SerializationUnit) -> type:
        cls = get_cls_from_name(serUnit)
        return cls

    def deserialize_dialect(self, serUnit: SerializationUnit) -> ir.Dialect:
        name = serUnit.data["name"]
        if name in self._ctx.Dialect_Lookup:
            return self._ctx.Dialect_Lookup[name]
        result = ir.Dialect(name)
        self._ctx.Dialect_Lookup[name] = result
        return result

    def deserialize_dialect_group(self, serUnit: SerializationUnit) -> ir.DialectGroup:
        dialects = self.deserialize_frozenset(serUnit.data["data"])
        cls = get_cls_from_name(serUnit)
        return cls(dialects=dialects)
