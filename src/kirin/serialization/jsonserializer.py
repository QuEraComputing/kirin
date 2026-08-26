import json
from typing import Any, Optional

from kirin.serialization.core.serializationunit import SerializationUnit
from kirin.serialization.core.serializationmodule import SerializationModule

COMPACT_UNIT_TAG = "$u"  # Compact SerializationUnit wrapper.
ESCAPED_MAP_TAG = "$m"  # Escaped singleton user mapping.

# Descriptors that are part of Kirin's core serialization vocabulary. A
# descriptor is omitted only when both strings match this table; extensions
# that reuse a kind with another class keep their full descriptor on the wire.
STATIC_DESCRIPTORS: dict[str, tuple[str, str]] = {
    "block": ("kirin.ir.nodes.block", "Block"),
    "block-arg": ("kirin.ir.ssa", "BlockArgument"),
    "block_ref": ("kirin.ir.nodes.block", "Block"),
    "bool": ("builtins", "bool"),
    "bytearray": ("builtins", "bytearray"),
    "bytes": ("builtins", "bytes"),
    "dialect": ("kirin.ir.dialect", "Dialect"),
    "dialect_group": ("kirin.ir.group", "DialectGroup"),
    "dict": ("builtins", "dict"),
    "float": ("builtins", "float"),
    "frozenset": ("builtins", "frozenset"),
    "int": ("builtins", "int"),
    "list": ("builtins", "list"),
    "method": ("kirin.ir.method", "Method"),
    "none": ("builtins", "NoneType"),
    "range": ("builtins", "range"),
    "region": ("kirin.ir.nodes.region", "Region"),
    "region_ref": ("kirin.ir.nodes.region", "Region"),
    "result-value": ("kirin.ir.ssa", "ResultValue"),
    "set": ("builtins", "set"),
    "slice": ("builtins", "slice"),
    "ssa_ref": ("", ""),
    "str": ("builtins", "str"),
    "tuple": ("builtins", "tuple"),
}


class JSONtifiable:
    """
    Helper class to convert between SerializationModule/SerializationUnit and JSON-serializable dicts.
    This is used internally by JSONSerializer and BSONSerializer to handle the actual conversion logic.
    """

    def _to_jsonifiable(self, obj: Any) -> Any:
        if isinstance(obj, SerializationModule):
            return {
                "__serialization_module__": True,
                "version": obj.version,
                "symbol_table": self._to_jsonifiable(obj.symbol_table),
                "body": self._to_jsonifiable(obj.body),
            }
        if isinstance(obj, SerializationUnit):
            data = self._to_jsonifiable(obj.data)
            descriptor = (obj.module_name, obj.class_name)
            if STATIC_DESCRIPTORS.get(obj.kind) == descriptor:
                return {COMPACT_UNIT_TAG: [obj.kind, data]}
            return {COMPACT_UNIT_TAG: [obj.kind, obj.module_name, obj.class_name, data]}
        if isinstance(obj, dict):
            encoded = {key: self._to_jsonifiable(value) for key, value in obj.items()}
            if len(encoded) == 1 and (
                COMPACT_UNIT_TAG in encoded or ESCAPED_MAP_TAG in encoded
            ):
                return {
                    ESCAPED_MAP_TAG: [[key, value] for key, value in encoded.items()]
                }
            return encoded
        if isinstance(obj, (list, tuple)):
            return [self._to_jsonifiable(v) for v in obj]
        return obj

    def _from_jsonifiable(self, obj: Any) -> Any:
        if isinstance(obj, dict):
            if obj.get("__serialization_module__"):
                symbol_table = self._from_jsonifiable(obj.get("symbol_table", {}))
                body = self._from_jsonifiable(obj.get("body"))
                version = obj.get("version", "")
                return SerializationModule(
                    symbol_table=symbol_table, body=body, version=version
                )
            if obj.get("__serialization_unit__"):
                data = self._from_jsonifiable(obj.get("data", {}))
                return SerializationUnit(
                    kind=obj["kind"],
                    module_name=obj["module_name"],
                    class_name=obj["class_name"],
                    data=data,
                )
            if len(obj) == 1 and COMPACT_UNIT_TAG in obj:
                return self._decode_compact_unit(obj[COMPACT_UNIT_TAG])
            if len(obj) == 1 and ESCAPED_MAP_TAG in obj:
                return self._decode_escaped_mapping(obj[ESCAPED_MAP_TAG])
            return {k: self._from_jsonifiable(v) for k, v in obj.items()}
        if isinstance(obj, list):
            return [self._from_jsonifiable(v) for v in obj]
        return obj

    def _decode_compact_unit(self, payload: Any) -> SerializationUnit:
        if not isinstance(payload, list) or len(payload) not in (2, 4):
            raise ValueError(
                f"{COMPACT_UNIT_TAG} payload must be a 2- or 4-item list, got {payload!r}"
            )

        kind = payload[0]
        if not isinstance(kind, str):
            raise ValueError(f"{COMPACT_UNIT_TAG} kind must be a string, got {kind!r}")

        if len(payload) == 2:
            descriptor = STATIC_DESCRIPTORS.get(kind)
            if descriptor is None:
                raise ValueError(
                    f"{COMPACT_UNIT_TAG} kind {kind!r} requires module/class descriptors"
                )
            module_name, class_name = descriptor
            raw_data = payload[1]
        else:
            module_name, class_name, raw_data = payload[1:]
            if not isinstance(module_name, str) or not isinstance(class_name, str):
                raise ValueError(
                    f"{COMPACT_UNIT_TAG} module/class descriptors must be strings, got "
                    f"{module_name!r} and {class_name!r}"
                )

        data = self._from_jsonifiable(raw_data)
        if not isinstance(data, dict):
            raise ValueError(
                f"{COMPACT_UNIT_TAG} data must decode to a mapping, got {data!r}"
            )
        return SerializationUnit(
            kind=kind,
            module_name=module_name,
            class_name=class_name,
            data=data,
        )

    def _decode_escaped_mapping(self, payload: Any) -> dict[str, Any]:
        if not isinstance(payload, list):
            raise ValueError(
                f"{ESCAPED_MAP_TAG} payload must be a list, got {payload!r}"
            )

        result: dict[str, Any] = {}
        for index, item in enumerate(payload):
            if not isinstance(item, list) or len(item) != 2:
                raise ValueError(
                    f"{ESCAPED_MAP_TAG} item {index} must be a 2-item list, got {item!r}"
                )
            key, value = item
            if not isinstance(key, str):
                raise ValueError(
                    f"{ESCAPED_MAP_TAG} item {index} key must be a string, got {key!r}"
                )
            if key in result:
                raise ValueError(f"{ESCAPED_MAP_TAG} contains duplicate key {key!r}")
            result[key] = self._from_jsonifiable(value)
        return result


class JSONSerializer(JSONtifiable):
    """
    JSON serializer/deserializer for SerializationModule
    and SerializationUnit.
    """

    def encode(self, data: SerializationModule) -> str:
        """
        Top-level function to encode a SerializationModule to a JSON string.
        Args:
            data: SerializationModule to encode.
        Returns:
            JSON string representation of the SerializationModule.
        """
        payload = self._to_jsonifiable(data)
        return json.dumps(payload, separators=(",", ":"), ensure_ascii=False)

    def decode(self, data: str) -> SerializationModule:
        """
        Top-level function to decode a JSON string to a SerializationModule.
        Args:
            data: JSON string to decode.
        Returns:
            Deserialized SerializationModule."""
        parsed = json.loads(data)
        result = self._from_jsonifiable(parsed)
        if not isinstance(result, SerializationModule):
            raise TypeError("decoded payload is not a SerializationModule")
        return result


_json_serializer_instance: Optional[JSONSerializer] = None


def get_json_serializer() -> JSONSerializer:
    """Lazily return a single JSONSerializer instance (module-level singleton)."""
    global _json_serializer_instance
    if _json_serializer_instance is None:
        _json_serializer_instance = JSONSerializer()
    return _json_serializer_instance
