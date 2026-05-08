import pytest

from kirin.prelude import basic
from kirin.serialization.jsonserializer import JSONSerializer
from kirin.serialization.base.serializer import Serializer
from kirin.serialization.core.serializationmodule import SerializationModule


@basic
def simple_kernel(x: int):
    return x + 1


def _empty_module(version: str = "") -> SerializationModule:
    encoded = basic.encode(simple_kernel)
    return SerializationModule(
        symbol_table=encoded.symbol_table, body=encoded.body, version=version
    )


def test_serialization_module_default_version_is_empty():
    mod = SerializationModule(symbol_table={}, body=basic.encode(simple_kernel).body)
    assert mod.version == ""


def test_serialization_module_stores_version():
    mod = _empty_module(version="1.2.3")
    assert mod.version == "1.2.3"


def test_check_version_match():
    mod = _empty_module(version="1.0.0")
    assert mod.check_version("1.0.0") is True


def test_check_version_mismatch():
    mod = _empty_module(version="1.0.0")
    assert mod.check_version("2.0.0") is False


def test_check_version_against_empty():
    mod = _empty_module(version="")
    assert mod.check_version("") is True
    assert mod.check_version("1.0.0") is False


def test_serializer_encode_default_version():
    serializer = Serializer()
    mod = serializer.encode(simple_kernel)
    assert mod.version == ""


def test_serializer_encode_propagates_version():
    serializer = Serializer()
    mod = serializer.encode(simple_kernel, version="2.5.0")
    assert mod.version == "2.5.0"


def test_dialect_group_encode_default_version():
    mod = basic.encode(simple_kernel)
    assert mod.version == ""


def test_dialect_group_encode_with_version():
    mod = basic.encode(simple_kernel, version="3.1.4")
    assert mod.version == "3.1.4"


def test_encode_json_round_trips_version():
    json_str = basic.encode_json(simple_kernel, version="0.9.0")
    decoded_module = JSONSerializer().decode(json_str)
    assert decoded_module.version == "0.9.0"


def test_decode_json_no_expected_version_succeeds():
    json_str = basic.encode_json(simple_kernel, version="1.0.0")
    method = basic.decode_json(json_str)
    assert method.code.is_structurally_equal(simple_kernel.code)


def test_decode_json_expected_version_match():
    json_str = basic.encode_json(simple_kernel, version="1.0.0")
    method = basic.decode_json(json_str, expect_version="1.0.0")
    assert method.code.is_structurally_equal(simple_kernel.code)


def test_decode_json_expected_version_mismatch_raises():
    json_str = basic.encode_json(simple_kernel, version="1.0.0")
    with pytest.raises(ValueError, match="Version mismatch"):
        basic.decode_json(json_str, expect_version="2.0.0")


def test_decode_json_expected_version_against_default_empty():
    json_str = basic.encode_json(simple_kernel)
    with pytest.raises(ValueError, match="Version mismatch"):
        basic.decode_json(json_str, expect_version="1.0.0")


def test_decode_json_expected_empty_matches_default():
    json_str = basic.encode_json(simple_kernel)
    method = basic.decode_json(json_str, expect_version="")
    assert method.code.is_structurally_equal(simple_kernel.code)
