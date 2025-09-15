from typing import Any
from importlib import import_module
from dataclasses import field

from kirin import ir, types
from kirin.dialects import func
from kirin.serialization.base.context import SerializationContext
from kirin.serialization.base.registry import (
    DIALECTS_LOOKUP,
    DialectSerializer,
    RuntimeSerializer,
    TypeAttributeSerializer,
    mangle,
    register_type,
    register_dialect,
    autodiscover_serializers,
)

BUILTINS = (bool, str, int, float, tuple, list, dict, slice, type(None))


class Serializer:
    _ctx: SerializationContext
    _runtime_serializer: RuntimeSerializer = field(default_factory=RuntimeSerializer)
    _typeattr_serializer: TypeAttributeSerializer = field(
        default_factory=TypeAttributeSerializer
    )
    _dialect_serializer: DialectSerializer = field(default_factory=DialectSerializer)

    def __init__(self, types: list[type] = []) -> None:
        self._ctx = SerializationContext()
        self._runtime_serializer = RuntimeSerializer()
        self._typeattr_serializer = TypeAttributeSerializer()
        self._dialect_serializer = DialectSerializer()
        self._ctx.Method_Symbol = getattr(self._ctx, "Method_Symbol", {})
        self._ctx.Method_Runtime = getattr(self._ctx, "Method_Runtime", {})
        register_type(ir.Method)
        for t in BUILTINS:
            register_type(t)
        autodiscover_serializers()
        for t in types:
            register_type(t)

    def encode(self, obj: object) -> dict[str, Any]:
        self._ctx.clear()
        body = self.serialize(obj)
        symbol_table = None
        if getattr(self._ctx, "Method_Symbol", None):
            st: dict[str, str] = {}
            for mangled, meta in self._ctx.Method_Symbol.items():
                st[mangled] = meta
            symbol_table = st or None
        else:
            symbol_table = None

        out = {"kind": "module", "symbol_table": symbol_table, "body": body}
        return out

    def decode(self, data: dict[str, Any]) -> Any:
        kind = data.get("kind")

        self._ctx._block_reference_store = {}
        for i in range(len(self._ctx.blk_idtable.lookup)):
            x = ir.Block.__new__(ir.Block)
            self._ctx._block_reference_store[i] = x
        if kind == "module":
            for mangled, meta in data["symbol_table"].items():
                if mangled in self._ctx.Method_Runtime:
                    continue
                if not isinstance(meta, dict):
                    continue
                sym_name = meta.get("sym_name")
                try:
                    m = ir.Method(
                        mod=None,
                        py_func=None,
                        sym_name=sym_name,
                        arg_names=[],
                        dialects=None,
                        code=None,
                    )
                except Exception:
                    m = ir.Method.__new__(ir.Method)
                    m.mod = None
                    m.py_func = None
                    m.sym_name = sym_name
                    m.arg_names = []
                    m.dialects = None
                    m.code = None
                encoded_arg_types = meta.get("arg_types", []) or []
                try:
                    decoded_arg_types = [
                        self._typeattr_serializer.decode(t_enc)
                        for t_enc in encoded_arg_types
                    ]
                    setattr(m, "arg_types", tuple(decoded_arg_types))
                except Exception:
                    pass

                self._ctx.Method_Runtime[mangled] = m
                self._ctx.Method_Symbol[mangled] = meta

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
        else:
            raise ValueError(f"Unsupported object type {type(obj)} for serialization.")

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
        elif kind == "attribute" or kind.startswith("attribute-"):
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

        mangled = mangle(mthd.sym_name, mthd.arg_types)
        meta = {
            "sym_name": mthd.sym_name,
            "arg_types": [self._typeattr_serializer.encode(t) for t in mthd.arg_types],
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

        existing = self._ctx.Method_Runtime.get(mangled)
        if existing is not None:
            out = existing
            out.sym_name = data["sym_name"]
            out.arg_names = data["arg_names"]
            out.dialects = self._dialect_serializer.decode(data["dialects"])
            out.code = self.deserialize(data["code"])
            sym_meta = self._ctx.Method_Symbol.get(mangled, {})
            encoded_arg_types = sym_meta.get("arg_types", []) or []
            if not encoded_arg_types:
                encoded_arg_types = []
            try:
                decoded_arg_types = tuple(
                    self._typeattr_serializer.decode(t_enc)
                    for t_enc in encoded_arg_types
                )
                if decoded_arg_types:
                    setattr(out, "arg_types", decoded_arg_types)
            except Exception:
                pass

        computed = mangle(out.sym_name, getattr(out, "arg_types", ()))
        if computed != mangled:
            raise ValueError(
                f"Mangled name mismatch: expected {mangled}, got {computed}"
            )

        self._ctx.Method_Runtime[mangled] = out
        if mangled not in self._ctx.Method_Symbol:
            self._ctx.Method_Symbol[mangled] = {
                "sym_name": out.sym_name,
                "arg_types": [
                    self._typeattr_serializer.encode(t)
                    for t in getattr(out, "arg_types", [])
                ],
            }
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
                callee_arg_types = getattr(callee, "arg_types", []) or []
                mangled = mangle(callee.sym_name, callee_arg_types)
                meta = {
                    "sym_name": callee.sym_name,
                    "arg_types": [
                        self._typeattr_serializer.encode(t) for t in callee_arg_types
                    ],
                }
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
        _results = list(self.deserialize(owner=out, data=x) for x in data["_results"])
        _name_args_slice = self.deserialize(data["_name_args_slice"])
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
            runtime = getattr(self._ctx, "Method_Runtime", {}) or {}
            if mangled_name not in runtime:
                meta = getattr(self._ctx, "Method_Symbol", {}) or {}
                entry = meta.get(mangled_name)
                if entry:
                    decoded_arg_types = [
                        self._typeattr_serializer.decode(t_enc)
                        for t_enc in entry.get("arg_types", [])
                    ]
                    placeholder = ir.Method(
                        mod=None,
                        py_func=None,
                        sym_name=entry.get("sym_name"),
                        arg_names=[],
                        dialects=None,
                        code=None,
                    )
                    setattr(placeholder, "arg_types", decoded_arg_types)
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
            if block_id in self._ctx._block_reference_store:
                block = self._ctx._block_reference_store.pop(block_id)
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
            if block_id in self._ctx._block_reference_store:
                out = self._ctx._block_reference_store.pop(block_id)
                self._ctx.Block_Lookup[block_id] = out
            else:
                out = ir.Block.__new__(ir.Block)
                self._ctx.Block_Lookup[block_id] = out
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
        out: dict[str, Any] = {}
        if isinstance(attr, ir.PyAttr):
            out["kind"] = "attribute-pyattr"
            val = attr.data
            if isinstance(val, types.TypeAttribute):
                out["data"] = {"__typeattr__": self._typeattr_serializer.encode(val)}
            elif type(val) in BUILTINS:
                out["data"] = self.serialize_builtin(val)
            elif hasattr(val, "serialize") and callable(getattr(val, "serialize")):
                out["data"] = val.serialize()
            out["pytype"] = self._typeattr_serializer.encode(attr.type)
            return out
        elif hasattr(attr, "serialize") and callable(getattr(attr, "serialize")):
            out["kind"] = "attribute-generic"
            out["module"] = attr.__class__.__module__
            out["name"] = attr.__class__.__name__
            out["data"] = self.serialize_builtin(attr.serialize())
            return out
        elif isinstance(attr, types.TypeAttribute):
            out["kind"] = "attribute-typeattr"
            out["data"] = self._typeattr_serializer.encode(attr)
            return out
        raise TypeError(
            f"Unsupported attribute type {type(attr)} for serialization. "
            "Provide a serialize()/deserialize() pair (implement SerializerMixin) "
            "or wrap Python values in PyAttr."
        )

    def deserialize_attribute(self, data: dict[str, Any]) -> ir.Attribute:
        kind = data.get("kind")

        if kind == "attribute-generic":
            mod_path = data.get("module")
            cls_name = data.get("name")
            payload_enc = data.get("data")

            if isinstance(payload_enc, dict) and payload_enc.get("kind") == "builtin":
                inner_payload = self.deserialize_builtin(payload_enc)
            else:
                inner_payload = payload_enc

            if not mod_path or not cls_name:
                raise ImportError(
                    f"Missing module/name for generic attribute: {data!r}"
                )

            mod = import_module(mod_path)
            try:
                attr_cls = getattr(mod, cls_name)
            except AttributeError:
                raise ImportError(
                    f"Attribute class {cls_name!r} not found in module {mod_path!r}"
                )

            if not hasattr(attr_cls, "deserialize") or not callable(
                getattr(attr_cls, "deserialize")
            ):
                raise ValueError(
                    f"Attribute class {cls_name} does not implement deserialize() method."
                )

            return attr_cls.deserialize(inner_payload)

        if kind == "attribute-pyattr":
            pytype_enc = data.get("pytype")
            pytype = (
                self._typeattr_serializer.decode(pytype_enc)
                if pytype_enc is not None
                else None
            )
            data_enc = data.get("data")

            if isinstance(data_enc, dict):
                if "__typeattr__" in data_enc:
                    decoded = self._typeattr_serializer.decode(data_enc["__typeattr__"])
                    return ir.PyAttr(
                        data=decoded,
                        pytype=(
                            decoded
                            if isinstance(decoded, types.TypeAttribute)
                            else pytype
                        ),
                    )
                elif data_enc.get("kind") == "builtin":
                    val = self.deserialize_builtin(data_enc)
                    return ir.PyAttr(data=val, pytype=pytype)
                elif hasattr(data_enc, "deserialize") and callable(
                    getattr(data_enc, "deserialize")
                ):
                    val = data_enc.deserialize()
                    return ir.PyAttr(data=val, pytype=pytype)
            return ir.PyAttr(data=data_enc, pytype=pytype)

        if kind == "attribute-typeattr":
            return self._typeattr_serializer.decode(data.get("data"))

        raise ValueError(f"Unknown attribute kind {kind}")

    def serialize_result(self, result: ir.ResultValue) -> dict[str, Any]:
        return {
            "kind": "result-value",
            "id": self._ctx.ssa_idtable[result],
            "index": result.index,
            "type": self._typeattr_serializer.encode(result.type),
            "name": result.name,
        }

    def deserialize_result(
        self, data: dict[str, Any], owner: ir.Statement | None = None
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
