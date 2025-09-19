from typing import Any, cast

from kirin import ir
from kirin.serialization.base.context import (
    MethodSymbolMeta,
    SerializationContext,
    mangle,
    get_str_from_type,
)


class Serializer:
    _ctx: SerializationContext

    def __init__(self, type_list: list[type] = []) -> None:
        self._ctx = SerializationContext()

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

    def serialize(self, obj: object) -> dict[str, Any]:
        if isinstance(obj, bool):
            return self.serialize_boolean(obj)
        elif isinstance(obj, bytes):
            return self.serialize_bytes(obj)
        elif isinstance(obj, bytearray):
            return self.serialize_bytes_array(obj)
        elif isinstance(obj, complex):
            return self.serialize_complex(obj)
        elif isinstance(obj, dict):
            return self.serialize_dict(obj)
        elif isinstance(obj, float):
            return self.serialize_float(obj)
        elif isinstance(obj, frozenset):
            return self.serialize_frozenset(obj)
        elif isinstance(obj, int):
            return self.serialize_int(obj)
        elif isinstance(obj, list):
            return self.serialize_list(obj)
        elif isinstance(obj, range):
            return self.serialize_range(obj)
        elif isinstance(obj, set):
            return self.serialize_set(obj)
        elif isinstance(obj, slice):
            return self.serialize_slice(obj)
        elif isinstance(obj, str):
            return self.serialize_str(obj)
        elif isinstance(obj, memoryview):
            return self.serialize_memoryview(obj)
        elif obj is None:
            return self.serialize_none(obj)
        elif isinstance(obj, tuple):
            return self.serialize_tuple(obj)
        elif isinstance(obj, type):
            return self.serialize_type(obj)
        elif isinstance(obj, ir.Method):
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
        elif isinstance(obj, ir.Dialect):
            return self.serialize_dialect(obj)
        elif isinstance(obj, ir.DialectGroup):
            return self.serialize_dialect_group(obj)
        elif hasattr(obj, "serialize") and callable(getattr(obj, "serialize")):
            return cast(Any, obj).serialize(self)
        else:
            raise ValueError(
                f"Unsupported object type {type(obj)} for serialization. Implement 'serialize' method."
            )

    def serialize_method(self, mthd: ir.Method) -> dict[str, Any]:

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
            "dialects": self.serialize_dialect_group(mthd.dialects),
            "code": self.serialize_statement(mthd.code),
            "mangled": mangled,
        }

    def serialize_statement(self, stmt: ir.Statement) -> dict[str, Any]:
        out = {
            "kind": "statement",
            "module": self.serialize_str(stmt.__class__.__module__),
            "class": self.serialize_str(stmt.__class__.__name__),
            "id": self._ctx.stmt_idtable[stmt],
            "dialect": self.serialize(stmt.dialect),
            "name": self.serialize_str(stmt.name),
            "_args": self.serialize_tuple(stmt._args),
            "_results": self.serialize_list(stmt._results),
            "_name_args_slice": self.serialize(stmt._name_args_slice),
            "attributes": self.serialize_dict(stmt.attributes),
            "successors": self.serialize_list(stmt.successors),
            "_regions": self.serialize_list(stmt._regions),
        }

        # if isinstance(stmt, func.Invoke):
        #     callee = stmt.callee
        #     if callee is not None:
        #         mangled = mangle(callee.sym_name, callee.arg_types, callee.return_type)
        #         if callee.sym_name is None:
        #             raise ValueError(
        #                 "Invoke.callee.sym_name is None, cannot serialize."
        #             )
        #         meta = MethodSymbolMeta(
        #             sym_name=callee.sym_name,
        #             arg_types=[t.__class__.__name__ for t in callee.arg_types],
        #             ret_type=callee.return_type,
        #         )
        #         if not hasattr(self._ctx, "Method_Symbol"):
        #             self._ctx.Method_Symbol = {}
        #         existing = self._ctx.Method_Symbol.get(mangled)
        #         if existing is None:
        #             self._ctx.Method_Symbol[mangled] = meta
        #         elif existing != meta:
        #             raise ValueError(
        #                 f"Mangled name collision for {mangled}: existing={existing} new={meta}"
        #             )

        #         out["call_method"] = mangled
        #     else:
        #         out["call_method"] = None
        # else:
        #     out["call_method"] = None

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

    def serialize_boolean(self, value: bool) -> dict[str, str]:
        return {
            "kind": "bool",
            "value": str(value) if value else "",
        }

    def serialize_bytes(self, value: bytes) -> dict[str, str]:
        return {"kind": "bytes", "value": value.hex()}

    def serialize_bytes_array(self, value: bytearray) -> dict[str, str]:
        return {"kind": "bytearray", "value": bytes(value).hex()}

    def serialize_complex(self, value: complex) -> dict[str, Any]:
        return {
            "kind": "complex",
            "real": str(value.real),
            "imag": str(value.imag),
        }

    def serialize_dict(self, value: dict) -> dict[str, Any]:
        return {
            "kind": "dict",
            "keys": [self.serialize(k) for k in value.keys()],
            "values": [self.serialize(v) for v in value.values()],
        }

    def serialize_float(self, value: float) -> dict[str, str]:
        return {
            "kind": "float",
            "value": str(value),
        }

    def serialize_frozenset(self, value: frozenset) -> dict[str, Any]:
        return {
            "kind": "frozenset",
            "value": [self.serialize(x) for x in value],
        }

    def serialize_int(self, value: int) -> dict[str, str]:
        return {
            "kind": "int",
            "value": str(value),
        }

    def serialize_list(self, value: list) -> dict[str, Any]:
        return {
            "kind": "list",
            "value": [self.serialize(x) for x in value],
        }

    def serialize_range(self, r: range) -> dict[str, Any]:
        return {
            "kind": "range",
            "start": self.serialize(r.start),
            "stop": self.serialize(r.stop),
            "step": self.serialize(r.step),
        }

    def serialize_set(self, value: set) -> dict[str, Any]:
        return {
            "kind": "set",
            "value": [self.serialize(x) for x in value],
        }

    def serialize_slice(self, value: slice) -> dict[str, Any]:
        return {
            "kind": "slice",
            "start": self.serialize(value.start),
            "stop": self.serialize(value.stop),
            "step": self.serialize(value.step),
        }

    def serialize_str(self, value: str) -> dict[str, str]:
        return {
            "kind": "str",
            "value": value,
        }

    def serialize_memoryview(self, value: memoryview) -> dict[str, Any]:
        return {"kind": "memoryview", "value": value.tobytes().hex()}

    def serialize_none(self, value: None) -> dict[str, str]:
        return {
            "kind": "none",
        }

    def serialize_tuple(self, value: tuple) -> dict[str, Any]:
        return {
            "kind": "tuple",
            "value": [self.serialize(x) for x in value],
        }

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

    def serialize_result(self, result: ir.ResultValue) -> dict[str, Any]:
        return {
            "kind": "result-value",
            "id": self._ctx.ssa_idtable[result],
            "owner": self.serialize_str(self._ctx.stmt_idtable[result.owner]),
            "index": result.index,
            "type": self.serialize_attribute(result.type),
            "name": result.name,
        }

    def serialize_type(self, typ: type) -> dict[str, Any]:
        return {
            "kind": "type",
            "module": typ.__module__,
            "name": typ.__name__,
        }

    def serialize_dialect(self, dialect: ir.Dialect) -> dict[str, Any]:
        return {
            "kind": "dialect",
            "name": self.serialize(dialect.name),
            "stmts": self.serialize(dialect.stmts),
        }

    def serialize_dialect_group(self, group: ir.DialectGroup) -> dict[str, Any]:
        return {
            "kind": "dialect_group",
            "data": self.serialize(group.data),
        }
