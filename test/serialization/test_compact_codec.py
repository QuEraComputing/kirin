import gzip
import json
from typing import Any
from collections.abc import Callable

import bson
import pytest

from kirin import ir
from kirin.serialization.bsonserializer import CompressedBSONSerializer
from kirin.serialization.jsonserializer import JSONtifiable, JSONSerializer
from kirin.serialization.core.serializationunit import SerializationUnit
from kirin.serialization.core.serializationmodule import SerializationModule

Transport = tuple[
    Callable[[SerializationModule], str | bytes],
    Callable[[str | bytes], SerializationModule],
    Callable[[str | bytes], dict[str, Any]],
]


def _json_transport() -> Transport:
    serializer = JSONSerializer()
    return serializer.encode, serializer.decode, json.loads


def _bson_transport() -> Transport:
    serializer = CompressedBSONSerializer()

    def load(payload: str | bytes) -> dict[str, Any]:
        assert isinstance(payload, bytes)
        return bson.decode(gzip.decompress(payload))

    return serializer.encode, serializer.decode, load


@pytest.fixture(params=[_json_transport, _bson_transport], ids=["json", "bson"])
def transport(request: pytest.FixtureRequest) -> Transport:
    return request.param()


def _module(body: SerializationUnit, *, version: str = "") -> SerializationModule:
    return SerializationModule(symbol_table={}, body=body, version=version)


def _assert_unit_equal(actual: SerializationUnit, expected: SerializationUnit) -> None:
    assert actual.kind == expected.kind
    assert actual.module_name == expected.module_name
    assert actual.class_name == expected.class_name
    _assert_value_equal(actual.data, expected.data)


def _assert_value_equal(actual: Any, expected: Any) -> None:
    if isinstance(expected, SerializationUnit):
        assert isinstance(actual, SerializationUnit)
        _assert_unit_equal(actual, expected)
        return
    if isinstance(expected, dict):
        assert isinstance(actual, dict)
        assert actual.keys() == expected.keys()
        for key, value in expected.items():
            _assert_value_equal(actual[key], value)
        return
    if isinstance(expected, (list, tuple)):
        assert isinstance(actual, (list, tuple))
        assert len(actual) == len(expected)
        for actual_item, expected_item in zip(actual, expected):
            _assert_value_equal(actual_item, expected_item)
        return
    assert actual == expected


STATIC_UNITS = [
    pytest.param("bool", "builtins", "bool", {"value": "True"}, id="bool"),
    pytest.param("bytes", "builtins", "bytes", {"value": "00ff"}, id="bytes"),
    pytest.param(
        "bytearray", "builtins", "bytearray", {"value": "00ff"}, id="bytearray"
    ),
    pytest.param("dict", "builtins", "dict", {"keys": [], "values": []}, id="dict"),
    pytest.param("float", "builtins", "float", {"value": "-0.0"}, id="float"),
    pytest.param("frozenset", "builtins", "frozenset", {"value": []}, id="frozenset"),
    pytest.param("int", "builtins", "int", {"value": "-7"}, id="int"),
    pytest.param("list", "builtins", "list", {"value": []}, id="list"),
    pytest.param("none", "builtins", "NoneType", {}, id="none"),
    pytest.param(
        "range",
        "builtins",
        "range",
        {
            "start": SerializationUnit("int", "builtins", "int", {"value": "1"}),
            "stop": SerializationUnit("int", "builtins", "int", {"value": "5"}),
            "step": SerializationUnit("int", "builtins", "int", {"value": "2"}),
        },
        id="range",
    ),
    pytest.param("set", "builtins", "set", {"value": []}, id="set"),
    pytest.param(
        "slice",
        "builtins",
        "slice",
        {
            "start": SerializationUnit("int", "builtins", "int", {"value": "1"}),
            "stop": SerializationUnit("none", "builtins", "NoneType", {}),
            "step": SerializationUnit("int", "builtins", "int", {"value": "-1"}),
        },
        id="slice",
    ),
    pytest.param("str", "builtins", "str", {"value": "Kirin"}, id="str"),
    pytest.param("tuple", "builtins", "tuple", {"value": []}, id="tuple"),
    pytest.param("method", ir.Method.__module__, ir.Method.__name__, {}, id="method"),
    pytest.param(
        "block-arg",
        ir.BlockArgument.__module__,
        ir.BlockArgument.__name__,
        {},
        id="block-arg",
    ),
    pytest.param("region", ir.Region.__module__, ir.Region.__name__, {}, id="region"),
    pytest.param(
        "region_ref", ir.Region.__module__, ir.Region.__name__, {}, id="region-ref"
    ),
    pytest.param("block", ir.Block.__module__, ir.Block.__name__, {}, id="block"),
    pytest.param(
        "block_ref", ir.Block.__module__, ir.Block.__name__, {}, id="block-ref"
    ),
    pytest.param(
        "result-value",
        ir.ResultValue.__module__,
        ir.ResultValue.__name__,
        {},
        id="result-value",
    ),
    pytest.param(
        "dialect", ir.Dialect.__module__, ir.Dialect.__name__, {}, id="dialect"
    ),
    pytest.param(
        "dialect_group",
        ir.DialectGroup.__module__,
        ir.DialectGroup.__name__,
        {},
        id="dialect-group",
    ),
]


@pytest.mark.parametrize(("kind", "module_name", "class_name", "data"), STATIC_UNITS)
def test_static_units_use_short_compact_shape_and_round_trip(
    transport: Transport,
    kind: str,
    module_name: str,
    class_name: str,
    data: dict[str, Any],
) -> None:
    encode, decode, load = transport
    unit = SerializationUnit(kind, module_name, class_name, data)

    payload = encode(_module(unit))
    wire = load(payload)

    assert wire["body"] == {"$u": [kind, wire["body"]["$u"][1]]}
    assert len(wire["body"]["$u"]) == 2
    decoded = decode(payload)
    _assert_unit_equal(decoded.body, unit)


DYNAMIC_UNITS = [
    pytest.param(
        "statement", "test.dialect", "ExampleStatement", {"id": "stmt0"}, id="statement"
    ),
    pytest.param(
        "attribute",
        "test.dialect",
        "ExampleAttribute",
        {"data": SerializationUnit("none", "builtins", "NoneType", {})},
        id="attribute",
    ),
    pytest.param("type", "test.types", "ExampleType", {}, id="type"),
    pytest.param(
        "dialect",
        "test.dialect",
        "ExampleDialect",
        {"name": "test"},
        id="dialect-subclass",
    ),
    pytest.param(
        "ilist", "test.extension", "ExampleIList", {"data": []}, id="extension"
    ),
    pytest.param("custom", "test.extension", "CustomUnit", {"answer": 42}, id="custom"),
    pytest.param(
        "int",
        "test.extension",
        "CustomInteger",
        {"value": "7"},
        id="known-kind-custom-descriptor",
    ),
]


@pytest.mark.parametrize(("kind", "module_name", "class_name", "data"), DYNAMIC_UNITS)
def test_dynamic_units_keep_descriptors_and_round_trip(
    transport: Transport,
    kind: str,
    module_name: str,
    class_name: str,
    data: dict[str, Any],
) -> None:
    encode, decode, load = transport
    unit = SerializationUnit(kind, module_name, class_name, data)

    payload = encode(_module(unit))
    wire = load(payload)

    assert wire["body"]["$u"][:3] == [kind, module_name, class_name]
    assert len(wire["body"]["$u"]) == 4
    decoded = decode(payload)
    _assert_unit_equal(decoded.body, unit)


@pytest.mark.parametrize(
    "data",
    [
        pytest.param({"$u": ["int", {"value": "5"}]}, id="unit-tag"),
        pytest.param({"$m": [["user", "value"]]}, id="map-tag"),
        pytest.param({"$u": {"$m": "nested-user-data"}}, id="nested-tags"),
    ],
)
def test_exact_control_tag_collisions_are_escaped_once_and_round_trip(
    transport: Transport, data: dict[str, Any]
) -> None:
    encode, decode, load = transport
    unit = SerializationUnit("custom", "test.extension", "CustomUnit", data)

    payload = encode(_module(unit))
    wire_data = load(payload)["body"]["$u"][3]

    assert list(wire_data) == ["$m"]
    assert wire_data["$m"][0][0] == next(iter(data))
    decoded = decode(payload)
    _assert_unit_equal(decoded.body, unit)


def test_nested_collisions_are_escaped_without_changing_ordinary_maps() -> None:
    codec = JSONtifiable()
    value = {
        "ordinary": {"$u": ["int", {"value": "5"}]},
        "nested": {"deeper": {"$m": [["user", "value"]]}},
    }

    wire = codec._to_jsonifiable(value)

    assert wire == {
        "ordinary": {"$m": [["$u", ["int", {"value": "5"}]]]},
        "nested": {
            "deeper": {"$m": [["$m", [["user", "value"]]]]},
        },
    }
    assert codec._from_jsonifiable(wire) == value


@pytest.mark.parametrize(
    "payload",
    [
        pytest.param({"$u": None}, id="unit-not-list"),
        pytest.param({"$u": []}, id="unit-empty"),
        pytest.param({"$u": ["int"]}, id="unit-too-short"),
        pytest.param({"$u": ["int", {}, "extra"]}, id="unit-length-three"),
        pytest.param(
            {"$u": ["int", "builtins", "int", {}, "extra"]}, id="unit-too-long"
        ),
        pytest.param({"$u": [1, {}]}, id="unit-kind-not-string"),
        pytest.param({"$u": ["unknown", {}]}, id="unknown-short-unit"),
        pytest.param({"$u": ["int", []]}, id="unit-data-not-map"),
        pytest.param({"$u": ["custom", 1, "Custom", {}]}, id="module-not-string"),
        pytest.param(
            {"$u": ["custom", "test.extension", 1, {}]}, id="class-not-string"
        ),
        pytest.param({"$m": None}, id="map-not-list"),
        pytest.param({"$m": ["not-a-pair"]}, id="map-entry-not-list"),
        pytest.param({"$m": [["key"]]}, id="map-pair-too-short"),
        pytest.param({"$m": [["key", 1, 2]]}, id="map-pair-too-long"),
        pytest.param({"$m": [[1, "value"]]}, id="map-key-not-string"),
        pytest.param({"$m": [["key", 1], ["key", 2]]}, id="duplicate-map-key"),
    ],
)
def test_malformed_exact_control_shapes_are_rejected(payload: dict[str, Any]) -> None:
    with pytest.raises((TypeError, ValueError)):
        JSONtifiable()._from_jsonifiable(payload)


@pytest.mark.parametrize("tag", ["$u", "$m"])
def test_control_key_with_siblings_is_an_ordinary_mapping(tag: str) -> None:
    value = {tag: "not-a-control-record", "user": True}
    assert JSONtifiable()._from_jsonifiable(value) == value


@pytest.mark.parametrize(
    "value",
    [
        pytest.param(-(2**63) - 1, id="below-int64"),
        pytest.param(-(2**63), id="int64-min"),
        pytest.param(2**63 - 1, id="int64-max"),
        pytest.param(2**63, id="above-int64"),
        pytest.param(2**4096 + 1, id="arbitrary-precision"),
    ],
)
def test_integer_payload_stays_a_string_through_public_transports(
    transport: Transport, value: int
) -> None:
    encode, decode, load = transport
    unit = SerializationUnit("int", "builtins", "int", {"value": str(value)})

    payload = encode(_module(unit))
    wire_value = load(payload)["body"]["$u"][1]["value"]

    assert wire_value == str(value)
    assert isinstance(wire_value, str)
    assert int(decode(payload).body.data["value"]) == value


@pytest.mark.parametrize(
    "version",
    [pytest.param("", id="empty"), pytest.param("caller-v9.4+🦉", id="arbitrary")],
)
def test_public_transports_preserve_caller_version(
    transport: Transport, version: str
) -> None:
    encode, decode, load = transport
    module = _module(
        SerializationUnit("none", "builtins", "NoneType", {}), version=version
    )

    payload = encode(module)

    assert load(payload)["version"] == version
    assert decode(payload).version == version


VERBOSE_INT = {
    "__serialization_unit__": True,
    "kind": "int",
    "module_name": "builtins",
    "class_name": "int",
    "data": {"value": str(2**256 + 1)},
}
VERBOSE_MODULE = {
    "__serialization_module__": True,
    "version": "caller-version",
    "symbol_table": {},
    "body": {
        "__serialization_unit__": True,
        "kind": "list",
        "module_name": "builtins",
        "class_name": "list",
        "data": {"value": [VERBOSE_INT]},
    },
}

VERBOSE_CONTROL_TAG_DATA = {
    "unit-shaped": {"$u": ["int", {"value": "5"}]},
    "map-shaped": {"$m": [["user", "value"]]},
}
VERBOSE_CONTROL_TAG_MODULE = {
    "__serialization_module__": True,
    "version": "caller-version",
    "symbol_table": {},
    "body": {
        "__serialization_unit__": True,
        "kind": "custom",
        "module_name": "test.extension",
        "class_name": "CustomUnit",
        "data": VERBOSE_CONTROL_TAG_DATA,
    },
}


@pytest.mark.parametrize("transport_name", ["json", "bson"])
def test_public_readers_accept_verbose_v1_and_reencode_compact(
    transport_name: str,
) -> None:
    if transport_name == "json":
        serializer = JSONSerializer()
        decoded = serializer.decode(json.dumps(VERBOSE_MODULE))
        compact_wire = json.loads(serializer.encode(decoded))
    else:
        serializer = CompressedBSONSerializer()
        payload = gzip.compress(bson.encode(VERBOSE_MODULE), mtime=0)
        decoded = serializer.decode(payload)
        compact_wire = bson.decode(gzip.decompress(serializer.encode(decoded)))

    assert decoded.version == "caller-version"
    assert decoded.body.kind == "list"
    assert decoded.body.data["value"][0].data["value"] == str(2**256 + 1)
    assert set(compact_wire["body"]) == {"$u"}


@pytest.mark.parametrize("transport_name", ["json", "bson"])
def test_verbose_v1_control_tag_shaped_mappings_remain_literal(
    transport_name: str,
) -> None:
    if transport_name == "json":
        module = JSONSerializer().decode(json.dumps(VERBOSE_CONTROL_TAG_MODULE))
    else:
        payload = gzip.compress(bson.encode(VERBOSE_CONTROL_TAG_MODULE), mtime=0)
        module = CompressedBSONSerializer().decode(payload)

    assert module.body.data == VERBOSE_CONTROL_TAG_DATA


def test_missing_verbose_version_keeps_the_existing_empty_default() -> None:
    payload = dict(VERBOSE_MODULE)
    payload.pop("version")

    decoded = JSONSerializer().decode(json.dumps(payload))

    assert decoded.version == ""
