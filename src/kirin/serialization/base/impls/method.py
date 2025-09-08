from kirin import ir
from kirin.serialization.base.registry import (
    RuntimeSerializer,
    register_type,
    runtime_register_decode,
    runtime_register_encode,
)

register_type(ir.Method)


@runtime_register_encode(ir.Method)
def _encode_method(encoder: RuntimeSerializer, obj: ir.Method):
    dialects = []
    if isinstance(obj.dialects, ir.Dialect):
        dialects = [obj.dialects]
    elif isinstance(obj.dialects, ir.DialectGroup):
        dialects = [d.name for d in obj.dialects.data]
    return {
        "kind": "method",
        "sym_name": obj.sym_name,
        "arg_names": obj.arg_names,
        "dialects": dialects,
        "code": encoder.encode(obj.code),
    }


@runtime_register_decode(ir.Method)
def _decode_method(decoder: RuntimeSerializer, data):
    return ir.Method(
        sym_name=data["sym_name"],
        arg_names=data["arg_names"],
        dialects=data["dialects"],
        code=decoder.decode(data["code"]),
    )
