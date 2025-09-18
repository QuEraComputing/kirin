import json
from typing import Any

from kirin.serialization.base.serializer import Serializer
from kirin.serialization.base.deserializer import Deserializer


class JSONSerializer:
    def __init__(self, types: list[type] = []):
        self.serializer = Serializer(type_list=types)
        self.deserializer = Deserializer(type_list=types)

    def encode(self, obj: object) -> dict[str, Any]:
        return self.serializer.encode(obj)

    def encode_to_str(self, obj: object, **json_kwargs) -> str:
        return json.dumps(
            self.encode(obj), separators=(",", ":"), indent=2, **json_kwargs
        )

    def decode(self, data: dict[str, Any]) -> Any:
        return self.deserializer.decode(data)

    def decode_from_str(self, payload: str) -> Any:
        parsed = json.loads(payload)
        return self.decode(parsed)
