import gzip
from typing import Optional

import bson

from kirin.serialization.core.serializationmodule import SerializationModule

from .jsonserializer import JSONtifiable


class CompressedBSONSerializer(JSONtifiable):

    def encode(self, data: SerializationModule) -> bytes:
        """
        Top-level function to encode a SerializationModule to a BSON byte string.
        Args:
            data: SerializationModule to encode.
        Returns:
            BSON byte string representation of the SerializationModule.
        """
        payload = self._to_jsonifiable(data)
        return gzip.compress(bson.encode(payload))

    def decode(self, data: bytes) -> SerializationModule:
        """
        Top-level function to decode a BSON byte string to a SerializationModule.
        Args:
            data: BSON byte string to decode.
        Returns:
            Deserialized SerializationModule."""
        parsed = bson.decode(gzip.decompress(data))
        result = self._from_jsonifiable(parsed)
        if not isinstance(result, SerializationModule):
            raise TypeError("decoded payload is not a SerializationModule")
        return result


_bson_serializer_instance: Optional[CompressedBSONSerializer] = None


def get_bson_serializer() -> CompressedBSONSerializer:
    """Lazily return a single CompressedBSONSerializer instance (module-level singleton)."""
    global _bson_serializer_instance
    if _bson_serializer_instance is None:
        _bson_serializer_instance = CompressedBSONSerializer()
    return _bson_serializer_instance
