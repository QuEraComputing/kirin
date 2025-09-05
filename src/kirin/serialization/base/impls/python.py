from kirin.serialization.base.registry import (
    RuntimeSerializer,
    register_type,
    runtime_register_decode,
    runtime_register_encode,
)

## pytype:

register_type(str)
register_type(bool)
register_type(int)
register_type(float)
register_type(type(None))


@runtime_register_encode(str)
@runtime_register_encode(bool)
@runtime_register_encode(int)
@runtime_register_encode(float)
@runtime_register_encode(type(None))
def encode_pytype(encoder: RuntimeSerializer, obj):
    return obj


@runtime_register_decode(float)
def decode_pyfloat(decoder: RuntimeSerializer, data):
    return float(data)


@runtime_register_decode(int)
def decode_pyint(decoder: RuntimeSerializer, data):
    return int(data)


@runtime_register_decode(bool)
def decode_pybool(decoder: RuntimeSerializer, data):
    return bool(data)


@runtime_register_decode(str)
def decode_pystr(decoder: RuntimeSerializer, data):
    return str(data)


# py sequence
register_type(list)
register_type(tuple)
register_type(dict)


@runtime_register_encode(list)
def encode_pylist(encoder: RuntimeSerializer, obj):
    return [encoder.encode(elem) for elem in obj]


@runtime_register_decode(list)
def decode_pylist(decoder: RuntimeSerializer, data):
    return [decoder.decode(elem) for elem in data]


@runtime_register_encode(tuple)
def encode_pytuple(encoder: RuntimeSerializer, obj):
    return tuple(encoder.encode(elem) for elem in obj)


@runtime_register_decode(tuple)
def decode_pytuple(decoder: RuntimeSerializer, data):
    return tuple(decoder.decode(elem) for elem in data)


@runtime_register_encode(dict)
def encode_pydict(encoder: RuntimeSerializer, obj):
    return {
        "keys": [encoder.encode(k) for k in obj.keys()],
        "values": [encoder.encode(v) for v in obj.values()],
    }


@runtime_register_decode(dict)
def decode_pydict(decoder: RuntimeSerializer, data):
    return {
        decoder.decode(k): decoder.decode(v)
        for k, v in zip(data["keys"], data["values"])
    }
