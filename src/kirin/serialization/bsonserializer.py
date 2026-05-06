from typing import Optional

import bson

from kirin.serialization.core.serializationmodule import SerializationModule

from .jsonserializer import JSONtifiable


class BSONSerializer(JSONtifiable):

    def encode(self, data: SerializationModule) -> bytes:
        """
        Top-level function to encode a SerializationModule to a BSON byte string.
        Args:
            data: SerializationModule to encode.
        Returns:
            BSON byte string representation of the SerializationModule.
        """
        payload = self._to_jsonifiable(data)
        return bson.encode(payload)

    def decode(self, data: bytes) -> SerializationModule:
        """
        Top-level function to decode a BSON byte string to a SerializationModule.
        Args:
            data: BSON byte string to decode.
        Returns:
            Deserialized SerializationModule."""
        parsed = bson.decode(data)
        result = self._from_jsonifiable(parsed)
        if not isinstance(result, SerializationModule):
            raise TypeError("decoded payload is not a SerializationModule")
        return result


_bson_serializer_instance: Optional[BSONSerializer] = None


def get_bson_serializer() -> BSONSerializer:
    """Lazily return a single BSONSerializer instance (module-level singleton)."""
    global _bson_serializer_instance
    if _bson_serializer_instance is None:
        _bson_serializer_instance = BSONSerializer()
    return _bson_serializer_instance
