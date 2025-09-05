import json

from kirin import ir
from kirin.serialization.base.serializer import Serializer


class JSONSerializer(Serializer):

    def encode(self, obj) -> str:
        data = super().serialize(obj)
        return json.dumps(data, separators=(",", ":"), indent=2)

    def decode(self, payload: str) -> ir.Method:
        data = json.loads(payload)
        return super().deserialize(data)
