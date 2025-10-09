import typing
from collections.abc import Sequence

from kirin import ir, types
from kirin.dialects.func.attrs import Signature
from kirin.serialization.base.context import (
    MethodSymbolMeta,
    SerializationContext,
    mangle,
    get_str_from_type,
)
from kirin.serialization.base.serializable import Serializable
from kirin.serialization.base.serializationunit import SerializationUnit
from kirin.serialization.base.serializationmodule import SerializationModule


class Serializer:
    _ctx: SerializationContext

    def __init__(self) -> None:
        self._ctx = SerializationContext()
        self._ctx.clear()

    def encode(self, obj: ir.Method) -> SerializationModule:
        body = self.serialize_method(obj)
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
        return SerializationModule(symbol_table=symbol_table, body=body)

    def serialize(
        self, obj: Serializable | Sequence[typing.Any] | None
    ) -> SerializationUnit:
        if isinstance(obj, ir.Attribute):
            return self.serialize_attribute(obj)

        ser_method = getattr(
            self, "serialize_" + type(obj).__name__.lower(), self.generic_serialize
        )
        return ser_method(obj)

    def generic_serialize(self, obj: object) -> SerializationUnit:
        if isinstance(obj, bool):
            return self.serialize_boolean(obj)
        elif isinstance(obj, bytes):
            return self.serialize_bytes(obj)
        elif isinstance(obj, bytearray):
            return self.serialize_bytes_array(obj)
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
        elif obj is None:
            return self.serialize_none(obj)
        elif isinstance(obj, tuple):
            return self.serialize_tuple(obj)
        elif isinstance(obj, type):
            return self.serialize_type(obj)
        else:
            raise ValueError(
                f"Unsupported object type {type(obj)} for serialization. Implement 'serialize' method."
            )

    def serialize_method(self, mthd: ir.Method) -> SerializationUnit:

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

        out = SerializationUnit(
            kind="method",
            module_name=ir.Method.__module__,
            class_name=ir.Method.__name__,
            data={
                "sym_name": mthd.sym_name,
                "arg_names": mthd.arg_names,
                "dialects": self.serialize_dialect_group(mthd.dialects),
                "code": self.serialize_statement(mthd.code),
                "nargs": self.serialize_int(mthd.nargs),
                "mangled": mangled,
            },
        )
        return out

    def serialize_statement(self, stmt: ir.Statement) -> SerializationUnit:
        return SerializationUnit(
            kind="statement",
            module_name=stmt.__class__.__module__,
            class_name=stmt.__class__.__name__,
            data={
                "id": self._ctx.stmt_idtable[stmt],
                "dialect": self.serialize(stmt.dialect),
                "name": self.serialize_str(stmt.name),
                "_args": self.serialize_tuple(stmt._args),
                "_results": self.serialize_list(stmt._results),
                "_name_args_slice": self.serialize_dict(stmt._name_args_slice),
                "attributes": self.serialize_dict(stmt.attributes),
                "successors": self.serialize_list(stmt.successors),
                "_regions": self.serialize_list(stmt._regions),
            },
        )

    def serialize_blockargument(self, arg: ir.BlockArgument) -> SerializationUnit:
        return SerializationUnit(
            kind="block-arg",
            module_name=ir.BlockArgument.__module__,
            class_name=ir.BlockArgument.__name__,
            data={
                "kind": "block-arg",
                "id": self._ctx.ssa_idtable[arg],
                "blk_id": self._ctx.blk_idtable[arg.owner],
                "index": arg.index,
                "type": self.serialize_attribute(arg.type),
                "name": arg.name,
            },
        )

    def serialize_region(self, region: ir.Region) -> SerializationUnit:
        region_id = self._ctx.region_idtable[region]
        if region_id in self._ctx.Region_Lookup:
            return SerializationUnit(
                kind="region_ref",
                module_name=ir.Region.__module__,
                class_name=ir.Region.__name__,
                data={
                    "kind": "region_ref",
                    "id": region_id,
                },
            )
        else:
            self._ctx.Region_Lookup[region_id] = region
            return SerializationUnit(
                kind="region",
                module_name=ir.Region.__module__,
                class_name=ir.Region.__name__,
                data={
                    "id": region_id,
                    "blocks": [self.serialize(block) for block in region.blocks],
                },
            )

    def serialize_block(self, block: ir.Block) -> SerializationUnit:
        if self._ctx.blk_idtable[block] in self._ctx.Block_Lookup:
            return SerializationUnit(
                kind="block_ref",
                module_name=ir.Block.__module__,
                class_name=ir.Block.__name__,
                data={
                    "id": self._ctx.blk_idtable[block],
                },
            )
        else:
            self._ctx.Block_Lookup[self._ctx.blk_idtable[block]] = block
            return SerializationUnit(
                kind="block",
                module_name=ir.Block.__module__,
                class_name=ir.Block.__name__,
                data={
                    "id": self._ctx.blk_idtable[block],
                    "stmts": [self.serialize_statement(stmt) for stmt in block.stmts],
                    "_args": [self.serialize_blockargument(arg) for arg in block.args],
                },
            )

    def serialize_boolean(self, value: bool) -> SerializationUnit:
        return SerializationUnit(
            kind="bool",
            module_name=bool.__module__,
            class_name=bool.__name__,
            data={
                "value": str(value) if value else "",
            },
        )

    def serialize_bytes(self, value: bytes) -> SerializationUnit:
        return SerializationUnit(
            kind="bytes",
            module_name=bytes.__module__,
            class_name=bytes.__name__,
            data={
                "value": value.hex(),
            },
        )

    def serialize_bytes_array(self, value: bytearray) -> SerializationUnit:
        return SerializationUnit(
            kind="bytearray",
            module_name=bytearray.__module__,
            class_name=bytearray.__name__,
            data={
                "value": bytes(value).hex(),
            },
        )

    def serialize_dict(self, value: dict) -> SerializationUnit:
        return SerializationUnit(
            kind="dict",
            module_name=dict.__module__,
            class_name=dict.__name__,
            data={
                "keys": [self.serialize(k) for k in value.keys()],
                "values": [self.serialize(v) for v in value.values()],
            },
        )

    def serialize_float(self, value: float) -> SerializationUnit:
        return SerializationUnit(
            kind="float",
            module_name=float.__module__,
            class_name=float.__name__,
            data={
                "value": str(value),
            },
        )

    def serialize_frozenset(self, value: frozenset) -> SerializationUnit:
        return SerializationUnit(
            kind="frozenset",
            module_name=frozenset.__module__,
            class_name=frozenset.__name__,
            data={
                "value": [self.serialize(x) for x in value],
            },
        )

    def serialize_int(self, value: int) -> SerializationUnit:
        return SerializationUnit(
            kind="int",
            module_name=int.__module__,
            class_name=int.__name__,
            data={
                "value": str(value),
            },
        )

    def serialize_list(self, value: list) -> SerializationUnit:
        return SerializationUnit(
            kind="list",
            module_name=list.__module__,
            class_name=list.__name__,
            data={
                "value": [self.serialize(x) for x in value],
            },
        )

    def serialize_range(self, r: range) -> SerializationUnit:
        return SerializationUnit(
            kind="range",
            module_name=range.__module__,
            class_name=range.__name__,
            data={
                "start": self.serialize_int(r.start),
                "stop": self.serialize_int(r.stop),
                "step": self.serialize_int(r.step),
            },
        )

    def serialize_set(self, value: set) -> SerializationUnit:
        return SerializationUnit(
            kind="set",
            module_name=set.__module__,
            class_name=set.__name__,
            data={
                "value": [self.serialize(x) for x in value],
            },
        )

    def serialize_slice(self, value: slice) -> SerializationUnit:
        return SerializationUnit(
            kind="slice",
            module_name=slice.__module__,
            class_name=slice.__name__,
            data={
                "start": self.serialize(value.start),
                "stop": self.serialize(value.stop),
                "step": self.serialize(value.step),
            },
        )

    def serialize_str(self, value: str) -> SerializationUnit:
        return SerializationUnit(
            kind="str",
            module_name=str.__module__,
            class_name=str.__name__,
            data={
                "value": value,
            },
        )

    def serialize_none(self, value: None) -> SerializationUnit:
        return SerializationUnit(
            kind="none",
            module_name=type(value).__module__,
            class_name=type(value).__name__,
            data={},
        )

    def serialize_tuple(self, value: tuple) -> SerializationUnit:
        return SerializationUnit(
            kind="tuple",
            module_name=tuple.__module__,
            class_name=tuple.__name__,
            data={
                "value": [self.serialize(x) for x in value],
            },
        )

    def serialize_attribute(self, attr: ir.Attribute) -> SerializationUnit:
        if not isinstance(attr, Serializable):
            raise TypeError(f"Attribute {attr} is not Serializable.")
        return SerializationUnit(
            kind="attribute",
            module_name=attr.__module__,
            class_name=attr.__class__.__name__,
            data={"data": attr.serialize(self)},
        )

    def serialize_resultvalue(self, result: ir.ResultValue) -> SerializationUnit:
        return SerializationUnit(
            kind="result-value",
            module_name=ir.ResultValue.__module__,
            class_name=ir.ResultValue.__name__,
            data={
                "kind": "result-value",
                "id": self._ctx.ssa_idtable[result],
                "owner": self.serialize_str(self._ctx.stmt_idtable[result.owner]),
                "index": result.index,
                "type": self.serialize_attribute(result.type),
                "name": result.name,
            },
        )

    def serialize_type(self, typ: type) -> SerializationUnit:
        return SerializationUnit(
            kind="type", module_name=typ.__module__, class_name=typ.__name__, data={}
        )

    def serialize_dialect(self, dialect: ir.Dialect) -> SerializationUnit:
        return SerializationUnit(
            kind="dialect",
            module_name=dialect.__module__,
            class_name=dialect.__class__.__name__,
            data={"name": dialect.name},
        )

    def serialize_dialect_group(self, group: ir.DialectGroup) -> SerializationUnit:
        return SerializationUnit(
            kind="dialect_group",
            module_name=ir.DialectGroup.__module__,
            class_name=ir.DialectGroup.__name__,
            data={
                "data": self.serialize_frozenset(group.data),
            },
        )

    def serialize_pyclass(self, pc: types.PyClass) -> SerializationUnit:
        return SerializationUnit(
            kind="pyclass",
            module_name=pc.__module__,
            class_name=pc.__class__.__name__,
            data=dict(
                typ=self.serialize_type(pc.typ),
                display_name=self.serialize_str(pc.display_name),
                prefix=self.serialize_str(pc.prefix),
            ),
        )

    def serialize_typevar(self, tv: types.TypeVar) -> SerializationUnit:
        return SerializationUnit(
            kind="typevar",
            module_name=tv.__module__,
            class_name=tv.__class__.__name__,
            data={
                "varname": self.serialize_str(tv.varname),
                "bound": self.serialize_attribute(tv.bound),
            },
        )

    def serialize_generic(self, gen: types.Generic) -> SerializationUnit:
        return SerializationUnit(
            kind="generic",
            module_name=gen.__module__,
            class_name=gen.__class__.__name__,
            data={
                "body": self.serialize_pyclass(gen.body),
                "vars": self.serialize_tuple(gen.vars),
                "vararg": self.serialize(gen.vararg),
            },
        )

    def serialize_vararg(self, var: types.Vararg) -> SerializationUnit:
        return SerializationUnit(
            kind="vararg",
            module_name=var.__module__,
            class_name=var.__class__.__name__,
            data={"typ": self.serialize_attribute(var.typ)},
        )

    def serialize_anytype(self, at: types.AnyType) -> SerializationUnit:
        return SerializationUnit(
            kind="anytype",
            module_name=at.__module__,
            class_name=at.__class__.__name__,
            data=dict(),
        )

    def serialize_bottomtype(self, bt: types.BottomType) -> SerializationUnit:
        return SerializationUnit(
            kind="bottomtype",
            module_name=bt.__module__,
            class_name=bt.__class__.__name__,
            data=dict(),
        )

    def serialize_pyattr(self, pa: ir.PyAttr) -> SerializationUnit:
        return SerializationUnit(
            kind="pyattr",
            module_name=pa.__module__,
            class_name=pa.__class__.__name__,
            data={
                "data": self.serialize(pa.data),
                "pytype": self.serialize_attribute(pa.type),
            },
        )

    def serialize_literal(self, lit: types.Literal) -> SerializationUnit:
        return SerializationUnit(
            kind="literal",
            module_name=lit.__module__,
            class_name=lit.__class__.__name__,
            data={
                "value": self.serialize(lit.data),
                "type": self.serialize_attribute(lit.type),
            },
        )

    def serialize_union(self, uni: types.Union) -> SerializationUnit:
        return SerializationUnit(
            kind="Union",
            module_name=uni.__module__,
            class_name=uni.__class__.__name__,
            data={"types": self.serialize_frozenset(uni.types)},
        )

    def serialize_signature(self, sig: Signature) -> SerializationUnit:
        return SerializationUnit(
            kind="signature",
            module_name=sig.__module__,
            class_name=sig.__class__.__name__,
            data={
                "inputs": self.serialize_tuple(sig.inputs),
                "output": self.serialize(sig.output),
            },
        )
