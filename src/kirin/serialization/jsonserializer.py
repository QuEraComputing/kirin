import json

from kirin import ir
from kirin.serialization.base.serializer import Serializer


class JSONSerializer(Serializer):

    def encode(self, mthd: ir.Method) -> str:
        data = super().serialize_method(mthd)
        return json.dumps(data, separators=(",", ":"), indent=2)

    def decode(self, payload: str) -> ir.Method:
        data = json.loads(payload)
        return super().deserialize_method(data)
