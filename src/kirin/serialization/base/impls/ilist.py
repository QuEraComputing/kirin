from kirin.dialects import ilist
from kirin.serialization.base.registry import (
    RuntimeSerializer,
    register_type,
    runtime_register_decode,
    runtime_register_encode,
)

register_type(ilist.IList)


@runtime_register_encode(ilist.IList)
def encode_ilist(encoder: RuntimeSerializer, obj):
    return {"data": [encoder.encode(elem) for elem in obj.data]}


@runtime_register_decode(ilist.IList)
def decode_ilist(decoder: RuntimeSerializer, data):
    return ilist.IList(data=[decoder.decode(elem) for elem in data["data"]])
