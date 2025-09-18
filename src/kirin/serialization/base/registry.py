from typing import Any, Dict
from dataclasses import dataclass

from kirin import ir

RUNTIME_ENCODE_LOOKUP: Dict[str, Any] = {}
RUNTIME_DECODE_LOOKUP: Dict[str, Any] = {}
RUNTIME_NAME2TYPE: Dict[str, type] = {}
DIALECTS_LOOKUP = {}


def register_dialect(dialect: ir.Dialect):
    stmt_map: dict[str, type] = {}
    for stmt_cls in dialect.stmts:
        stmt_map[stmt_cls.__name__] = stmt_cls
        stmt_declared_name = getattr(stmt_cls, "name", None)
        if stmt_declared_name and stmt_declared_name != stmt_cls.__name__:
            stmt_map[stmt_declared_name] = stmt_cls

    DIALECTS_LOOKUP[dialect.name] = (dialect, stmt_map)


@dataclass
class DialectSerializer:
    def encode(self, obj: ir.Dialect | ir.DialectGroup | None):
        if isinstance(obj, ir.DialectGroup):
            return self.encode_dialect_group(obj)
        elif isinstance(obj, ir.Dialect):
            return self.encode_dialect(obj)
        else:
            raise ValueError(f"Unsupported dialect type {type(obj)} for encoding.")

    def decode(self, data: dict):
        if "kind" not in data:
            raise ValueError("Invalid dialect data for decoding.")

        match data["kind"]:
            case "dialect":
                return self.decode_dialect(data)
            case "dialect_group":
                return self.decode_dialect_group(data)
            case _:
                raise ValueError(
                    f"Unsupported dialect kind {data['kind']} for decoding."
                )

    def encode_dialect_group(self, group: ir.DialectGroup) -> dict:
        return {
            "kind": "dialect_group",
            "dialects": [self.encode_dialect(dialect) for dialect in group.data],
        }

    def decode_dialect_group(self, data: dict) -> ir.DialectGroup:
        if data.get("kind") != "dialect_group":
            raise ValueError("Not a dialect group data for decoding.")

        dialects = [
            self.decode_dialect(dialect_data) for dialect_data in data["dialects"]
        ]
        return ir.DialectGroup(dialects=dialects)

    def encode_dialect(self, obj: ir.Dialect):
        if obj.name not in DIALECTS_LOOKUP:
            raise ValueError(f"No registered dialect for {obj.name}. {DIALECTS_LOOKUP}")

        return {
            "kind": "dialect",
            "name": obj.name,
        }

    def decode_dialect(self, data: dict):
        if data.get("kind") != "dialect":
            raise ValueError("Not a dialect data for decoding.")

        name = data.get("name")
        if name not in DIALECTS_LOOKUP:
            raise ValueError(f"No registered dialect for name {name}.")

        return DIALECTS_LOOKUP[name][0]
