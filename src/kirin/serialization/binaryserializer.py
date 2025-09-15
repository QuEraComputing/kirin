import pickle
from typing import Any

from kirin.serialization.base.serializer import Serializer


class BinarySerializer(Serializer):
    def __init__(self, types: list[type] = []):
        super().__init__(types=types)

    def encode(self, obj) -> bytes:
        data = super().encode(obj)
        return pickle.dumps(data, protocol=pickle.HIGHEST_PROTOCOL)

    def decode(self, payload: bytes) -> Any:
        data = pickle.loads(payload)
        return super().decode(data)
