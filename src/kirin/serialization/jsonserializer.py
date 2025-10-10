import json
from typing import Any

from kirin.serialization.core.serializationunit import SerializationUnit
from kirin.serialization.core.serializationmodule import SerializationModule


class JSONSerializer:
    """
    JSON serializer/deserializer for SerializationModule
    and SerializationUnit.
    """

    def _to_jsonifiable(self, obj: Any) -> Any:
        if isinstance(obj, SerializationModule):
            return {
                "__serialization_module__": True,
                "symbol_table": self._to_jsonifiable(obj.symbol_table),
                "body": self._to_jsonifiable(obj.body),
            }
        if isinstance(obj, SerializationUnit):
            return {
                "__serialization_unit__": True,
                "kind": obj.kind,
                "module_name": obj.module_name,
                "class_name": obj.class_name,
                "data": self._to_jsonifiable(obj.data),
            }
        if isinstance(obj, dict):
            return {k: self._to_jsonifiable(v) for k, v in obj.items()}
        if isinstance(obj, (list, tuple)):
            return [self._to_jsonifiable(v) for v in obj]
        return obj

    def _from_jsonifiable(self, obj: Any) -> Any:
        if isinstance(obj, dict):
            if obj.get("__serialization_module__"):
                symbol_table = self._from_jsonifiable(obj.get("symbol_table", {}))
                body = self._from_jsonifiable(obj.get("body"))
                return SerializationModule(symbol_table=symbol_table, body=body)
            if obj.get("__serialization_unit__"):
                data = self._from_jsonifiable(obj.get("data", {}))
                return SerializationUnit(
                    kind=obj["kind"],
                    module_name=obj["module_name"],
                    class_name=obj["class_name"],
                    data=data,
                )
            return {k: self._from_jsonifiable(v) for k, v in obj.items()}
        if isinstance(obj, list):
            return [self._from_jsonifiable(v) for v in obj]
        return obj

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
