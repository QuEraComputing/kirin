import json
from typing import Any

from kirin.serialization.base.serializer import Serializer


class JSONSerializer(Serializer):
    def __init__(self, types: list[type] = []):
        super().__init__(types=types)

    def encode(self, obj: object) -> dict[str, Any]:
        return super().encode(obj)

    def encode_to_str(self, obj: object, **json_kwargs) -> str:
        return json.dumps(
            self.encode(obj), separators=(",", ":"), indent=2, **json_kwargs
        )

    def decode(self, data: dict[str, Any]) -> Any:
        return super().decode(data)

    def decode_from_str(self, payload: str) -> Any:
        parsed = json.loads(payload)
        return self.decode(parsed)
