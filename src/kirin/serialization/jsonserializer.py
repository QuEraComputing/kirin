import json

from kirin import ir
from kirin.serialization.base.serializer import Serializer


class JSONSerializer(Serializer):

    def encode(self, obj):
        data = super().encode(obj)
        return json.dumps(data, separators=(",", ":"), indent=2)

    def decode(self, payload) -> ir.Method:
        data = json.loads(payload)
        return super().decode(data)
