import pickle

from kirin import ir
from kirin.serialization.base.serializer import Serializer


class BinarySerializer(Serializer):

    def encode(self, obj) -> bytes:
        data = super().serialize(obj)
        return pickle.dumps(data, protocol=pickle.HIGHEST_PROTOCOL)

    def decode(self, payload: bytes) -> ir.Method:
        data = pickle.loads(payload)
        return super().deserialize(data)
