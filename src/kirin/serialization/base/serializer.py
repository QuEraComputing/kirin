from typing import Any, cast
from importlib import import_module
from dataclasses import field

import kirin.types as types
from kirin import ir
from kirin.dialects import func
from kirin.serialization.base.context import (
    MethodSymbolMeta,
    SerializationContext,
    mangle,
    get_str_from_type,
)
from kirin.serialization.base.registry import (
    DIALECTS_LOOKUP,
    DialectSerializer,
    register_type,
    register_dialect,
    autodiscover_serializers,
)

BUILTINS = (bool, str, int, float, tuple, list, dict, slice, type(None))


class Serializer:
    _ctx: SerializationContext
    _dialect_serializer: DialectSerializer = field(default_factory=DialectSerializer)

    def __init__(self, types: list[type] = []) -> None:
        self._ctx = SerializationContext()
        self._dialect_serializer = DialectSerializer()
        register_type(ir.Method)
        for t in BUILTINS:
            register_type(t)
        autodiscover_serializers()
        for t in types:
            register_type(t)

    def encode(self, obj: object) -> dict[str, Any]:
        self._ctx.clear()
        body = self.serialize(obj)
        if getattr(self._ctx, "Method_Symbol", None):
            st: dict[str, MethodSymbolMeta] = {}
            for mangled, meta in self._ctx.Method_Symbol.items():
                sym_name = meta.get("sym_name")
                if sym_name is None:
                    raise ValueError(f"symbol_table[{mangled}] missing 'sym_name'")
                st[mangled] = (
                    MethodSymbolMeta(
                        sym_name=sym_name,
                        arg_types=meta.get("arg_types", []),
                    )
                    if isinstance(meta, dict)
                    else meta
                )
            symbol_table: dict[str, MethodSymbolMeta] = st
        else:
            symbol_table = dict[str, MethodSymbolMeta]()

        out = {"kind": "module", "symbol_table": symbol_table, "body": body}
        return out

    def decode(self, data: dict[str, Any]) -> Any:
        kind = data.get("kind")
        self._ctx._block_reference_store = {}

        if kind == "module":
            for mangled, meta in data["symbol_table"].items():
                if not isinstance(meta, dict):
                    raise TypeError(f"symbol_table[{mangled}] is not a dict")
                sym_name = meta.get("sym_name")
                if sym_name is None:
                    raise ValueError(f"symbol_table[{mangled}] missing 'sym_name'")
                arg_types = meta.get("arg_types", []) or []
                self._ctx.Method_Symbol[mangled] = MethodSymbolMeta(
                    sym_name=sym_name,
                    arg_types=list(arg_types),
                )

            body = data.get("body")
            if body is None:
                raise ValueError("Module envelope missing body for decoding.")
            return self.deserialize(body)

        return self.deserialize(data)

    def serialize(self, obj: object) -> dict[str, Any]:
        if isinstance(obj, ir.Method):
            return self.serialize_method(obj)
        elif isinstance(obj, ir.BlockArgument):
            return self.serialize_block_argument(obj)
        elif isinstance(obj, ir.Statement):
            return self.serialize_statement(obj)
        elif isinstance(obj, ir.Region):
            return self.serialize_region(obj)
        elif isinstance(obj, ir.Attribute):
            return self.serialize_attribute(obj)
        elif isinstance(obj, ir.Block):
            return self.serialize_block(obj)
        elif isinstance(obj, ir.ResultValue):
            return self.serialize_result(obj)
        elif type(obj) in BUILTINS:
            return self.serialize_builtin(obj)
        elif hasattr(obj, "serialize") and callable(getattr(obj, "serialize")):
            return cast(Any, obj).serialize(self)
        else:
            raise ValueError(
                f"Unsupported object type {type(obj)} for serialization. Implement 'serialize' method."
            )

    def deserialize(
        self, data: dict[str, Any], owner: ir.Statement | None = None
    ) -> Any:
        kind = data.get("kind")
        if kind is None:
            raise ValueError("Invalid data for deserialization: missing 'kind' field.")
        if kind == "method":
            return self.deserialize_method(data)
        elif kind == "block-arg":
            return self.deserialize_block_argument(data)
        elif kind == "statement":
            return self.deserialize_statement(data)
        elif kind == "region":
            return self.deserialize_region(data)
        elif kind == "region_ref":
            return self.deserialize_region(data)
        elif kind == "attribute":
            return self.deserialize_attribute(data)
        elif kind == "block" or kind == "block_ref":
            return self.deserialize_block(data)
        elif kind == "result-value":
            return self.deserialize_result(data, owner=owner)
        elif kind == "builtin":
            return self.deserialize_builtin(data)
        else:
            raise ValueError(f"Unsupported data kind {kind} for deserialization.")

    def serialize_method(self, mthd: ir.Method) -> dict[str, Any]:
        method_dialects = mthd.dialects
        if isinstance(method_dialects, ir.Dialect):
            register_dialect(method_dialects)
        elif isinstance(method_dialects, ir.DialectGroup):
            for d in method_dialects.data:
                register_dialect(d)

        mangled = mangle(
            mthd.sym_name,
            getattr(mthd, "arg_types", ()),
            getattr(mthd, "ret_type", None),
        )
        arg_types_list: list[str] = []
        ret_type: str = get_str_from_type(mthd.return_type)
        for t in getattr(mthd, "arg_types", ()):
            arg_types_list.append(get_str_from_type(t))
        if mthd.sym_name is None:
            raise ValueError("Method.sym_name is None, cannot serialize.")
        meta: MethodSymbolMeta = {
            "sym_name": mthd.sym_name,
            "arg_types": arg_types_list,
            "ret_type": ret_type,
        }

        existing = self._ctx.Method_Symbol.get(mangled)
        if existing is not None:
            if existing != meta:
                raise ValueError(
                    f"Mangled name collision for {mangled}: existing={existing} new={meta}"
                )
        else:
            self._ctx.Method_Symbol[mangled] = meta

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

        mangled = data.get("mangled")
        if mangled is None:
            raise ValueError("Missing 'mangled' key for method deserialization.")

        out = self._ctx.Method_Runtime.get(mangled)
        if out is None:
            out = ir.Method.__new__(ir.Method)
            out.mod = None
            out.py_func = None
            out.code = ir.Statement.__new__(ir.Statement)
            self._ctx.Method_Runtime[mangled] = out

        out.sym_name = data["sym_name"]
        out.arg_names = data.get("arg_names", [])
        out.dialects = self._dialect_serializer.decode(data["dialects"])
        out.code = self.deserialize(data["code"])

        sym_meta = self._ctx.Method_Symbol.get(mangled, {}) or {}
        if isinstance(sym_meta, dict):
            encoded_arg_types = data.get("arg_types", []) or []
        else:
            encoded_arg_types = []
        try:
            decoded_arg_types = tuple(
                t_enc.deserialize(self) for t_enc in encoded_arg_types
            )
            if decoded_arg_types:
                try:
                    setattr(out, "arg_types", decoded_arg_types)
                except (AttributeError, TypeError):
                    object.__setattr__(out, "_arg_types", tuple(decoded_arg_types))
        except Exception:
            pass

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

    def serialize_statement(self, stmt: ir.Statement) -> dict[str, Any]:
        dialects = stmt.dialect
        out = {
            "kind": "statement",
            "dialect": self._dialect_serializer.encode(dialects),
            "name": stmt.name,
            "_args": [self.serialize(arg) for arg in stmt._args],
            "_results": [self.serialize(res) for res in stmt._results],
            "_name_args_slice": self.serialize(stmt._name_args_slice),
            "attributes": {k: self.serialize(v) for k, v in stmt.attributes.items()},
            "successors": [self.serialize(succ) for succ in stmt.successors],
            "_regions": [self.serialize(region) for region in stmt._regions],
        }

        if isinstance(stmt, func.Invoke):
            callee = stmt.callee
            if callee is not None:
                mangled = mangle(callee.sym_name, callee.arg_types, callee.return_type)
                if callee.sym_name is None:
                    raise ValueError(
                        "Invoke.callee.sym_name is None, cannot serialize."
                    )
                meta = MethodSymbolMeta(
                    sym_name=callee.sym_name,
                    arg_types=[t.__class__.__name__ for t in callee.arg_types],
                    ret_type=callee.return_type,
                )
                if not hasattr(self._ctx, "Method_Symbol"):
                    self._ctx.Method_Symbol = {}
                existing = self._ctx.Method_Symbol.get(mangled)
                if existing is None:
                    self._ctx.Method_Symbol[mangled] = meta
                elif existing != meta:
                    raise ValueError(
                        f"Mangled name collision for {mangled}: existing={existing} new={meta}"
                    )

                out["call_method"] = mangled
            else:
                out["call_method"] = None
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
        _results = list(
            self.deserialize_result(owner=out, data=x) for x in data["_results"]
        )
        _name_args_slice = self.deserialize(data["_name_args_slice"])
        _attributes = {
            k: self.deserialize_attribute(v) for k, v in data["attributes"].items()
        }

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
            runtime = self._ctx.Method_Runtime
            if mangled_name not in runtime:
                method_meta = self._ctx.Method_Symbol[mangled_name]
                if method_meta:
                    decoded_arg_types = []
                    placeholder = ir.Method.__new__(ir.Method)
                    placeholder.mod = None
                    placeholder.py_func = None
                    placeholder.sym_name = method_meta.get("sym_name")
                    placeholder.arg_names = []
                    placeholder.dialects = ir.DialectGroup([])
                    placeholder.code = ir.Statement.__new__(ir.Statement)
                    try:
                        setattr(placeholder, "arg_types", decoded_arg_types)
                    except (AttributeError, TypeError):
                        object.__setattr__(
                            placeholder, "_arg_types", tuple(decoded_arg_types)
                        )
                    self._ctx.Method_Runtime[mangled_name] = placeholder
                else:
                    raise ValueError(
                        f"Method with mangled name {mangled_name} not found."
                    )
            out.callee = self._ctx.Method_Runtime[mangled_name]

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
            "type": self.serialize_attribute(arg.type),
            "name": arg.name,
        }
        return out

    def deserialize_block_argument(self, data: dict[str, Any]) -> ir.BlockArgument:
        if data.get("kind") != "block-arg":
            raise ValueError("Invalid SSA block argument data for decoding.")

        ssa_name = data["id"]
        if ssa_name in self._ctx.SSA_Lookup:
            existing = self._ctx.SSA_Lookup[ssa_name]
            if isinstance(existing, ir.BlockArgument):
                return existing
            raise ValueError(
                f"Block argument id {ssa_name} already present but maps to {type(existing).__name__}"
            )

        blk_name = data["blk_id"]
        block = self._ctx.Block_Lookup.get(blk_name)
        if block is None:
            if blk_name in self._ctx._block_reference_store:
                block = self._ctx._block_reference_store.pop(blk_name)
                self._ctx.Block_Lookup[blk_name] = block
            else:
                block = ir.Block.__new__(ir.Block)
                self._ctx.Block_Lookup[blk_name] = block

        index = data["index"]
        typ = self.deserialize_attribute(data["type"])
        if not isinstance(typ, types.TypeAttribute):
            raise TypeError(f"Expected a TypeAttribute, got {type(typ)!r}: {typ!r}")
        out = ir.BlockArgument(
            block=block, index=index, type=cast(types.TypeAttribute, typ)
        )
        out._name = data.get("name", None)
        self._ctx.SSA_Lookup[ssa_name] = out

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
            region_name = data.get("id")
            if region_name is not None:
                self._ctx.Region_Lookup[region_name] = out

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
            region_name = data["id"]
            if region_name not in self._ctx.Region_Lookup:
                raise ValueError(f"Region with id {region_name} not found in lookup.")
            return self._ctx.Region_Lookup[region_name]
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

        block_name = block_data["id"]
        if block_name not in self._ctx.Block_Lookup:
            raise ValueError(f"Block with id {block_name} not found in lookup.")
        return self._ctx.Block_Lookup[block_name]

    def deserialize_concrete_block(self, block_data: dict) -> ir.Block:
        if block_data.get("kind") != "block":
            raise ValueError("Invalid block data for decoding.")

        block_name = block_data["id"]

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

    def serialize_builtin(self, obj: Any) -> dict[str, Any]:
        out: dict[str, Any] = {
            "kind": "builtin",
            "type": type(obj).__name__,
        }
        if obj is None or isinstance(obj, str):
            out["value"] = obj
            return out
        elif isinstance(obj, bool):
            out["value"] = str(obj) if obj else ""
            return out
        elif isinstance(obj, (int, float)):
            out["value"] = str(obj)
            return out

        elif isinstance(obj, dict):
            dict_value: dict[str, Any] = {
                "keys": [self.serialize(k) for k in obj.keys()],
                "values": [self.serialize(v) for v in obj.values()],
            }
            out["value"] = dict_value
            return out

        elif isinstance(obj, list):
            out["value"] = [self.serialize(x) for x in obj]
            return out

        elif isinstance(obj, tuple):
            out["value"] = [self.serialize(x) for x in obj]
            return out

        elif isinstance(obj, slice):
            out["value"] = {
                "start": self.serialize(obj.start),
                "stop": self.serialize(obj.stop),
                "step": self.serialize(obj.step),
            }
            return out

        else:
            raise TypeError(f"Unsupported builtin type {type(obj)} for serialization.")

    def deserialize_builtin(self, data: dict[str, Any]) -> Any:
        if data.get("kind") != "builtin":
            raise ValueError("Invalid builtin data for deserialization.")
        if data["type"] == "NoneType":
            return None
        elif data["type"] == "str":
            return data["value"]
        elif data["type"] == "int":
            return int(data["value"])
        elif data["type"] == "float":
            return float(data["value"])
        elif data["type"] == "bool":
            return bool(data["value"])
        elif data["type"] == "dict":
            dict_value = data["value"]
            keys = [self.deserialize(k) for k in dict_value.get("keys", [])]
            values = [self.deserialize(v) for v in dict_value.get("values", [])]
            return dict(zip(keys, values))
        elif data["type"] == "list":
            return [self.deserialize(x) for x in data.get("value", [])]
        elif data["type"] == "tuple":
            return tuple(self.deserialize(x) for x in data.get("value", []))
        elif data["type"] == "slice":
            slice_value = data["value"]
            start = self.deserialize(slice_value.get("start"))
            stop = self.deserialize(slice_value.get("stop"))
            step = self.deserialize(slice_value.get("step"))
            return slice(start, stop, step)
        else:
            raise TypeError(
                f"Unsupported builtin type {data['type']} for deserialization."
            )

    def serialize_attribute(self, attr: ir.Attribute) -> dict[str, Any]:
        if hasattr(attr, "serialize") and callable(getattr(attr, "serialize")):
            return {
                "kind": "attribute",
                "module": attr.__class__.__module__,
                "name": attr.__class__.__name__,
                "data": attr.serialize(self),
            }
        raise TypeError(
            f"Unsupported attribute type {type(attr)} for serialization. "
            "Provide a serialize()/deserialize() pair (implement SerializerMixin) "
        )

    def deserialize_attribute(self, data: dict[str, Any]) -> ir.Attribute:
        module_name = data.get("module")
        class_name = data.get("name")
        if not module_name or not class_name:
            raise ValueError(
                f"Attribute {data} must contain 'module' and 'name' fields."
            )

        mod = import_module(module_name)
        cls = getattr(mod, class_name, None)
        if cls is None:
            raise ImportError(f"Could not find class {class_name} in {module_name}")

        payload = data.get("data")
        if payload is None:
            raise ValueError("Attribute data missing 'data' field for deserialization.")

        if getattr(cls, "deserialize", None) and callable(getattr(cls, "deserialize")):
            try:
                return cls.deserialize(payload, self)
            except TypeError:
                return cls.deserialize(payload)
        else:
            raise ValueError(f"Class {cls} missing deserialize() method.")

    def serialize_result(self, result: ir.ResultValue) -> dict[str, Any]:
        return {
            "kind": "result-value",
            "id": self._ctx.ssa_idtable[result],
            "index": result.index,
            "type": self.serialize_attribute(result.type),
            "name": result.name,
        }

    def deserialize_result(
        self, data: dict[str, Any], owner: ir.Statement | None = None
    ) -> ir.ResultValue:
        if data.get("kind") != "result-value":
            raise ValueError("Invalid result SSA data for decoding.")
        ssa_name = data["id"]
        if ssa_name in self._ctx.SSA_Lookup:
            existing = self._ctx.SSA_Lookup[ssa_name]
            if isinstance(existing, ir.ResultValue):
                return existing
            raise ValueError(
                f"SSA id {ssa_name} already exists and is {type(existing).__name__}"
            )
        index = int(data["index"])

        typ = self.deserialize_attribute(data["type"])
        if owner is None:
            raise ValueError(
                "Owner (Statement) must not be None when deserializing a ResultValue."
            )
        if typ is None or not isinstance(typ, types.TypeAttribute):
            raise TypeError(f"Expected a TypeAttribute, got {type(typ)!r}: {typ!r}")
        out = ir.ResultValue(
            stmt=owner, index=index, type=cast(types.TypeAttribute, typ)
        )
        out.name = data.get("name", None)

        self._ctx.SSA_Lookup[ssa_name] = out

        return out
