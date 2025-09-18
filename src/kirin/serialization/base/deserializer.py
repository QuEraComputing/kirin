from typing import Any, cast
from importlib import import_module

import kirin.types as types
from kirin import ir
from kirin.dialects import func
from kirin.serialization.base.context import (
    MethodSymbolMeta,
    SerializationContext,
    mangle,
)
from kirin.serialization.base.registry import (
    DIALECTS_LOOKUP,
)


class Deserializer:
    _ctx: SerializationContext

    def __init__(self, type_list: list[type] = []) -> None:
        self._ctx = SerializationContext()

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

    def deserialize(
        self, data: dict[str, Any], owner: ir.Statement | None = None
    ) -> Any:
        match data["kind"]:
            case None:
                raise ValueError(
                    "Invalid data for deserialization: missing 'kind' field."
                )
            case "bool":
                return self.deserialize_boolean(data)
            case "bytes":
                return self.deserialize_bytes(data)
            case "bytearray":
                return self.deserialize_bytearray(data)
            case "complex":
                return self.deserialize_complex(data)
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
            case "memoryview":
                return self.deserialize_memoryview(data)
            case "none":
                return self.deserialize_none(data)
            case "tuple":
                return self.deserialize_tuple(data)
            case "type":
                return self.deserialize_type(data)
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
            case "dialect":
                return self.deserialize_dialect(data)
            case "dialect_group":
                return self.deserialize_dialect_group(data)
            case _:
                raise ValueError(
                    f"Unsupported data kind {data.get('kind')} for deserialization."
                )

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
        out.dialects = self.deserialize(data["dialects"])
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

    def deserialize_statement(self, data: dict[str, Any]) -> ir.Statement:
        if data.get("kind") != "statement":
            raise ValueError("Invalid statement data for decoding.")
        dialect: ir.Dialect = self.deserialize(data["dialect"])
        dialect_name = dialect.name
        tmp = DIALECTS_LOOKUP.get(dialect_name)
        # print(dialect.stmts)
        if tmp is None:
            raise ValueError(f"Dialect {dialect_name} not found in lookup.")

        dialect, stmt_map = tmp
        stmt_name = self.deserialize(data["name"])
        stmt_cls = stmt_map.get(stmt_name)
        # print()
        # print(stmt_cls)
        # print(dialect.stmts)
        # print(dialect.name, stmt_name)
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

    def deserialize_boolean(self, data: dict[str, str]) -> bool:
        if data.get("kind") != "bool":
            raise ValueError("Invalid boolean data for deserialization.")
        return bool(data["value"])

    def deserialize_bytes(self, data: dict[str, str]) -> bytes:
        if data.get("kind") != "bytes":
            raise ValueError("Invalid bytes data for deserialization.")
        return bytes.fromhex(data["value"])

    def deserialize_bytearray(self, data: dict[str, str]) -> bytearray:
        if data.get("kind") != "bytearray":
            raise ValueError("Invalid bytearray data for deserialization.")
        return bytearray.fromhex(data["value"])

    def deserialize_complex(self, data: dict[str, Any]) -> complex:
        if data.get("kind") != "complex":
            raise ValueError("Invalid complex data for deserialization.")
        return complex(data["real"], data["imag"])

    def deserialize_dict(self, data: dict[str, Any]) -> dict:
        if data.get("kind") != "dict":
            raise ValueError("Invalid dict data for deserialization.")
        keys = [self.deserialize(k) for k in data.get("keys", [])]
        values = [self.deserialize(v) for v in data.get("values", [])]
        return dict(zip(keys, values))

    def deserialize_float(self, data: dict[str, str]) -> float:
        if data.get("kind") != "float":
            raise ValueError("Invalid float data for deserialization.")
        return float(data["value"])

    def deserialize_frozenset(self, data: dict[str, Any]) -> frozenset:
        if data.get("kind") != "frozenset":
            raise ValueError("Invalid frozenset data for deserialization.")
        return frozenset(self.deserialize(x) for x in data.get("value", []))

    def deserialize_int(self, data: dict[str, str]) -> int:
        if data.get("kind") != "int":
            raise ValueError("Invalid int data for deserialization.")
        return int(data["value"])

    def deserialize_list(self, data: dict[str, Any]) -> list:
        if data.get("kind") != "list":
            raise ValueError("Invalid list data for deserialization.")
        return [self.deserialize(x) for x in data.get("value", [])]

    def deserialize_range(self, data: dict[str, Any]) -> range:
        if data.get("kind") != "range":
            raise ValueError("Invalid range data for deserialization.")
        start = self.deserialize(data.get("start", 0))
        stop = self.deserialize(data.get("stop", 0))
        step = self.deserialize(data.get("step", 1))
        if not all(isinstance(v, int) for v in (start, stop, step)):
            raise TypeError("Range start, stop, and step must be integers.")
        return range(start, stop, step)

    def deserialize_set(self, data: dict[str, Any]) -> set:
        if data.get("kind") != "set":
            raise ValueError("Invalid set data for deserialization.")
        return set(self.deserialize(x) for x in data.get("value", []))

    def deserialize_slice(self, data: dict[str, Any]) -> slice:
        if data.get("kind") != "slice":
            raise ValueError("Invalid slice data for deserialization.")
        start = self.deserialize(data["start"])
        stop = self.deserialize(data["stop"])
        step = self.deserialize(data["step"])
        return slice(start, stop, step)

    def deserialize_str(self, data: dict[str, str]) -> str:
        if data.get("kind") != "str":
            raise ValueError("Invalid string data for deserialization.")
        return data["value"]

    def deserialize_memoryview(self, data: dict[str, Any]) -> memoryview:
        if data.get("kind") != "memoryview":
            raise ValueError("Invalid memoryview data for deserialization.")
        return memoryview(bytes.fromhex(data["value"]))

    def deserialize_none(self, data: dict[str, str]) -> None:
        if data.get("kind") != "none":
            raise ValueError("Invalid NoneType data for deserialization.")
        return None

    def deserialize_tuple(self, data: dict[str, Any]) -> tuple:
        if data.get("kind") != "tuple":
            raise ValueError("Invalid tuple data for deserialization.")
        return tuple(self.deserialize(x) for x in data.get("value", []))

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
            return cls.deserialize(payload, self)
        else:
            raise ValueError(f"Class {cls} missing deserialize() method.")

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

    def deserialize_type(self, data: dict[str, Any]) -> type:
        if data.get("kind") != "type":
            raise ValueError("Invalid type data for deserialization.")
        module_name = data.get("module")
        class_name = data.get("name")
        if not module_name or not class_name:
            raise ValueError(f"Type {data} must contain 'module' and 'name' fields.")

        mod = import_module(module_name)
        cls = getattr(mod, class_name, None)

        if cls is None:
            if class_name == "NoneType" and module_name == "builtins":
                return type(None)
            else:
                raise ImportError(f"Could not find class {class_name} in {module_name}")
        return cls

    def deserialize_dialect(self, data: dict[str, Any]) -> ir.Dialect:
        if data.get("kind") != "dialect":
            raise ValueError("Not a dialect data for decoding.")

        name = self.deserialize(data["name"])
        # if name not in DIALECTS_LOOKUP:
        #     raise ValueError(f"No registered dialect for name {name}.")
        # return DIALECTS_LOOKUP[name][0]
        stmts = self.deserialize(data["stmts"])
        return ir.Dialect(name=name, stmts=stmts)

    def deserialize_dialect_group(self, data: dict) -> ir.DialectGroup:
        if data.get("kind") != "dialect_group":
            raise ValueError("Not a dialect group data for decoding.")
        dialects = self.deserialize(data["data"])
        return ir.DialectGroup(dialects=dialects)
