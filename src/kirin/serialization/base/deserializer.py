from typing import Any, cast

from kirin import ir, types
from kirin.dialects.func.attrs import Signature
from kirin.dialects.ilist.runtime import IList
from kirin.serialization.base.context import (
    MethodSymbolMeta,
    SerializationContext,
    mangle,
    get_cls_from_name,
)
from kirin.serialization.base.deserializable import Deserializable
from kirin.serialization.base.serializationunit import SerializationUnit
from kirin.serialization.base.serializationmodule import SerializationModule


class Deserializer:
    _ctx: SerializationContext

    def __init__(self) -> None:
        self._ctx = SerializationContext()
        self._ctx.clear()

    def decode(self, data: SerializationModule) -> ir.Method:
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
        ser_method = getattr(
            self, "serialize_" + serUnit.class_name.lower(), self.generic_deserialize
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
        out.dialects = self.deserialize_dialect_group(serUnit.data["dialects"])
        out.code = self.deserialize_statement(serUnit.data["code"])
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
        if not isinstance(typ, types.TypeAttribute):
            raise TypeError(f"Expected a TypeAttribute, got {type(typ)!r}: {typ!r}")
        out = cls(block=block, index=index, type=cast(types.TypeAttribute, typ))
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
        cls = get_cls_from_name(serUnit)
        if not issubclass(cls, Deserializable):
            raise TypeError(f"Class {cls} is not Deserializable.")
        assert issubclass(cls, ir.Attribute)
        return cls.deserialize(serUnit, self)

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

        typ = self.deserialize_attribute(serUnit.data["type"])
        if typ is None or not isinstance(typ, types.TypeAttribute):
            raise TypeError(f"Expected a TypeAttribute, got {type(typ)!r}: {typ!r}")
        owner: ir.Statement = self._ctx.Statement_Lookup[
            self.deserialize_str(serUnit.data["owner"])
        ]
        out = ir.ResultValue(
            stmt=owner, index=index, type=cast(types.TypeAttribute, typ)
        )
        out.name = serUnit.data.get("name", None)

        self._ctx.SSA_Lookup[ssa_name] = out

        return out

    def deserialize_type(self, serUnit: SerializationUnit) -> type:
        cls = get_cls_from_name(serUnit)
        return cls

    def deserialize_dialect(self, serUnit: SerializationUnit) -> ir.Dialect:
        name = self.deserialize_str(serUnit.data["name"])
        stmts = self.deserialize_list(serUnit.data["stmts"])
        cls = get_cls_from_name(serUnit)
        return cls(name=name, stmts=stmts)

    def deserialize_dialect_group(self, serUnit: SerializationUnit) -> ir.DialectGroup:
        dialects = self.deserialize_frozenset(serUnit.data["data"])
        cls = get_cls_from_name(serUnit)
        return cls(dialects=dialects)

    def deserialize_pyclass(self, serUnit: SerializationUnit) -> types.PyClass:
        return types.PyClass(
            typ=self.deserialize(serUnit.data["typ"]),
            display_name=self.deserialize(serUnit.data.get("display_name", "")),
            prefix=self.deserialize(serUnit.data.get("prefix", "")),
        )

    def deserialize_typevar(self, serUnit: SerializationUnit) -> types.TypeVar:
        varname = self.deserialize(serUnit.data["varname"])
        bound = self.deserialize(serUnit.data["bound"])
        return types.TypeVar(varname, bound)

    def deserialize_generic(self, serUnit: SerializationUnit) -> types.Generic:
        body = self.deserialize_pyclass(serUnit.data["body"])
        vars = self.deserialize_tuple(serUnit.data["vars"])
        vararg = self.deserialize(serUnit.data["vararg"])
        out = types.Generic(body, *vars)
        out.vararg = vararg
        return out

    def deserialize_vararg(self, serUnit: SerializationUnit) -> types.Vararg:
        typ = self.deserialize(serUnit.data["typ"])
        return types.Vararg(typ)

    def deserialize_anytype(self, serUnit: SerializationUnit) -> types.AnyType:
        return types.AnyType()

    def deserialize_bottomtype(self, serUnit: SerializationUnit) -> types.BottomType:
        return types.BottomType()

    def deserialize_pyattr(self, serUnit: SerializationUnit) -> ir.PyAttr:
        pytype = self.deserialize(serUnit.data["pytype"])
        value = self.deserialize(serUnit.data["data"])
        return ir.PyAttr(value, pytype=pytype)

    def deserialize_literal(self, serUnit: SerializationUnit) -> types.Literal:
        d = self.deserialize(serUnit.data["value"])
        type_attr = self.deserialize(serUnit.data["type"])
        return types.Literal(d, type_attr)

    def deserialize_union(self, serUnit: SerializationUnit) -> types.Union:
        ty = self.deserialize_frozenset(serUnit.data["types"])
        return types.Union(ty)

    def deserialize_signature(self, serUnit: SerializationUnit) -> Signature:
        inputs = self.deserialize(serUnit.data["inputs"])
        output = self.deserialize(serUnit.data["output"])
        return Signature(inputs=inputs, output=output)

    def deserialize_ilist(self, serUnit: SerializationUnit) -> IList:
        items = self.deserialize(serUnit.data["data"])
        elem = self.deserialize(serUnit.data["elem"])
        return IList(items, elem=elem)
